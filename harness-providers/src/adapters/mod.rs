//! Vendor adapters over the [`Provider`](crate::provider::Provider) trait.
//!
//! One shared HTTP/SSE engine ([`SseProvider`]) serves every vendor; a vendor
//! is a [`WireCodec`] — request rendering plus a per-stream frame codec —
//! and nothing else. All codecs parse ONLY each provider's documented stable
//! API surface; unknown fields are ignored, malformed frames are skipped and
//! logged (T9 policy), and adapter-specific extensions beyond the stable
//! surface must ship behind a per-provider cargo feature (none exist today).
//!
//! Adapters in this module:
//!
//! * [`anthropic::AnthropicAdapter`] — Messages API (`/v1/messages`).
//! * [`openai::OpenAiAdapter`] — chat completions and the Responses API.
//! * [`xai::XaiAdapter`] — Grok (chat-completions wire at `api.x.ai`).
//! * [`compat::CompatAdapter`] — any OpenAI-compatible base URL
//!   (Ollama/vLLM/OpenRouter/Groq).
//! * [`google::GoogleAdapter`] — Gemini `streamGenerateContent?alt=sse`.

pub mod anthropic;
pub mod chat;
pub mod compat;
pub mod google;
pub mod openai;
pub mod responses;
pub mod xai;

pub use anthropic::AnthropicAdapter;
pub use compat::CompatAdapter;
pub use google::GoogleAdapter;
pub use openai::OpenAiAdapter;
pub use xai::XaiAdapter;

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::auth::{AccessToken, AuthKind, CredentialSource};
use crate::error::ProviderStreamError;
use crate::event::ProviderEvent;
use crate::identity::{CompletionRequest, ModelIdentity};
use crate::provider::{EventSource, Provider, ProviderEventStream};
use crate::sse::{SseFrame, SseFrameParser};
use crate::transport::{Body, ByteStream, Transport, TransportRequest};
use crate::wire::MappedEvent;

/// Vendor request renderer + per-stream codec factory. One implementation
/// per wire family (chat-completions is shared by OpenAI, xAI, and the
/// compatible adapter).
pub(crate) trait WireCodec: Send + Sync {
    /// A pristine codec for one stream attempt.
    fn fresh(&self) -> Box<dyn StreamCodec>;
    /// Renders the HTTP request (URL, headers incl. auth, JSON body).
    fn render_request(&self, request: &CompletionRequest, auth: &AccessToken) -> TransportRequest;
}

/// Per-stream parsing state: vendor SSE frames in, typed events out.
pub(crate) trait StreamCodec: Send {
    /// Maps one frame; may consult per-stream state (tool-index maps,
    /// buffered finish reasons).
    fn feed(&mut self, frame: &SseFrame, configured: &ModelIdentity) -> Vec<MappedEvent>;
    /// Emits buffered terminal events at clean end-of-stream. Codecs that
    /// deliver `Finish` live (Anthropic, Gemini) return empty here; the
    /// chat-completions family holds `Finish` back until after the usage
    /// chunk, so it flushes.
    fn flush(&mut self, configured: &ModelIdentity) -> Vec<MappedEvent>;
}

/// A streaming provider over any vendor wire codec. This is the whole
/// adapter engine: auth header attach, transport send, HTTP error mapping,
/// SSE framing, codec dispatch.
pub struct SseProvider {
    transport: Arc<dyn Transport>,
    auth: Arc<dyn CredentialSource>,
    codec: Box<dyn WireCodec>,
    identity: ModelIdentity,
    kind: AuthKind,
}

impl SseProvider {
    /// Assembles a provider; `auth.kind()` is captured as the billing
    /// surface at construction.
    pub(crate) fn new(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        codec: Box<dyn WireCodec>,
        identity: ModelIdentity,
    ) -> Self {
        Self {
            kind: auth.kind(),
            transport,
            auth,
            codec,
            identity,
        }
    }

    /// The billing surface ([`AuthKind`]) this adapter serves — API key vs
    /// subscription — for the cost HUD (todo 42). Lives beside (not inside)
    /// [`ModelIdentity`]: identity equality is load-bearing in the turn
    /// layer's substitution checks, and an auth-mode switch must not
    /// fabricate an identity mismatch.
    #[must_use]
    pub fn auth_kind(&self) -> AuthKind {
        self.kind
    }
}

#[async_trait]
impl Provider for SseProvider {
    fn identity(&self) -> &ModelIdentity {
        &self.identity
    }

    async fn stream_completion(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderEventStream, ProviderStreamError> {
        let token = self
            .auth
            .credentials()
            .await
            .map_err(|err| ProviderStreamError::Auth(Box::new(err)))?;
        let http = self.codec.render_request(request, &token);
        let response = self
            .transport
            .send(http)
            .await
            .map_err(ProviderStreamError::Transport)?;
        if response.status >= 400 {
            let bytes = match response.body {
                Body::Full(bytes) => bytes,
                Body::Stream(mut stream) => read_error_body(&mut stream).await,
            };
            return Err(map_http_error(response.status, &bytes));
        }
        let input = match response.body {
            Body::Stream(stream) => BodyInput::Stream(stream),
            Body::Full(bytes) => BodyInput::Full(Some(bytes)),
        };
        Ok(ProviderEventStream::new(Box::new(VendorSseSource {
            input,
            parser: SseFrameParser::new(),
            codec: self.codec.fresh(),
            configured: self.identity.clone(),
            pending: VecDeque::new(),
        })))
    }
}

/// Body feeding source: chunked or fully-buffered.
enum BodyInput {
    /// Live chunk stream.
    Stream(ByteStream),
    /// One buffered payload delivered as a single chunk.
    Full(Option<Vec<u8>>),
}

impl BodyInput {
    async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        match self {
            Self::Stream(stream) => stream.next_chunk().await,
            Self::Full(pending) => pending.take(),
        }
    }
}

/// Typed event source over a vendor body: bytes → frames → codec → events.
pub(crate) struct VendorSseSource {
    input: BodyInput,
    parser: SseFrameParser,
    codec: Box<dyn StreamCodec>,
    configured: ModelIdentity,
    pending: VecDeque<Result<ProviderEvent, ProviderStreamError>>,
}

#[async_trait]
impl EventSource for VendorSseSource {
    async fn next(&mut self) -> Option<Result<ProviderEvent, ProviderStreamError>> {
        loop {
            if let Some(front) = self.pending.pop_front() {
                return Some(front);
            }
            let Some(chunk) = self.input.next_chunk().await else {
                for mapped in self.codec.flush(&self.configured) {
                    self.queue(mapped);
                }
                return self.pending.pop_front();
            };
            if let Err(failure) = self.parser.feed(&chunk) {
                return Some(Err(ProviderStreamError::Transport(failure)));
            }
            while let Some(frame) = self.parser.next_frame() {
                for mapped in self.codec.feed(&frame, &self.configured) {
                    self.queue(mapped);
                }
            }
        }
    }
}

impl VendorSseSource {
    fn queue(&mut self, mapped: MappedEvent) {
        match mapped {
            MappedEvent::Event(event) => self.pending.push_back(Ok(event)),
            MappedEvent::Error(err) => self.pending.push_back(Err(err)),
            MappedEvent::Skipped => {}
        }
    }
}

/// Stamps an event identity: the model the wire says it served wins over the
/// configured one (that is what makes silent substitution detectable).
pub(crate) fn stamp(configured: &ModelIdentity, served: Option<&str>) -> ModelIdentity {
    match served {
        Some(model) if model != configured.model => ModelIdentity {
            provider: configured.provider.clone(),
            model: model.to_owned(),
        },
        _ => configured.clone(),
    }
}

/// Builds the common HTTP request: JSON body, static headers, auth header
/// last (never overridable from request-level extras).
pub(crate) fn json_request(
    url: &str,
    body: &Value,
    auth: &AccessToken,
    static_headers: &[(&str, &str)],
) -> TransportRequest {
    let mut request = TransportRequest::new(url, serde_json::to_vec(body).unwrap_or_default());
    for (name, value) in static_headers {
        request = request.with_header(name, value);
    }
    if let Some((name, value)) = auth.style.header(&auth.value) {
        request = request.with_header(&name, &value);
    }
    request
}

/// Error bodies are untrusted: bounded to 64 KiB regardless of framing.
async fn read_error_body(stream: &mut ByteStream) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(chunk) = stream.next_chunk().await {
        if out.len() + chunk.len() > 64 * 1024 {
            tracing::warn!("provider error body exceeded 64 KiB; truncating");
            break;
        }
        out.extend_from_slice(&chunk);
    }
    out
}

/// Maps an HTTP error status + body to the typed stream error. Understands
/// the `{"error": {...}}` shapes of all five vendors (`type` — Anthropic,
/// string `code` — OpenAI/xAI, symbolic `status` or numeric `code` —
/// Google); anything else falls back to a truncated raw-body message.
/// Retry classification (429/5xx retryable, other 4xx fatal) stays with
/// [`ProviderStreamError::is_recoverable`].
pub(crate) fn map_http_error(status: u16, body: &[u8]) -> ProviderStreamError {
    let parsed: Option<Value> = serde_json::from_slice(body).ok();
    let detail = parsed.as_ref().and_then(|value| value.get("error"));
    let code = detail
        .and_then(|error| {
            error
                .get("type")
                .or_else(|| error.get("status"))
                .or_else(|| error.get("code"))
                .and_then(value_code)
        })
        .unwrap_or_else(|| "http_error".to_owned());
    let message = detail
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| truncate_body(body));
    ProviderStreamError::Http {
        status,
        code,
        message,
    }
}

/// Error-code fields arrive as strings (most vendors) or numbers (Google).
fn value_code(value: &Value) -> Option<String> {
    match value {
        Value::String(code) => Some(code.clone()),
        Value::Number(code) => Some(code.to_string()),
        _ => None,
    }
}

fn truncate_body(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    text.chars().take(300).collect()
}
