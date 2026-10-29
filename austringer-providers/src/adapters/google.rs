//! Google Gemini adapter
//! (`POST /v1beta/models/{model}:streamGenerateContent?alt=sse`).
//!
//! Wire mapping: `candidates[0].content.parts[].text` → text;
//! `parts[].functionCall` → tool calls with SYNTHESIZED stable ids
//! (`gemini-call-{n}` — the API provides none; ids are stable within the
//! response and correlate start/delta pairs, which is what the T9 assembly
//! requires); `finishReason` STOP→ToolCall when function calls were seen,
//! else EndTurn, MAX_TOKENS→Length, safety stops→Refusal;
//! `usageMetadata` → cache-aware usage.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{SseProvider, StreamCodec, WireCodec, json_request, stamp};
use crate::auth::{AccessToken, AuthKind, CredentialSource};
use crate::error::ProviderStreamError;
use crate::event::{FinishReason, ProviderEvent, Usage};
use crate::identity::{CompletionRequest, ModelIdentity};
use crate::provider::{Provider, ProviderEventStream};
use crate::sse::SseFrame;
use crate::transport::{Transport, TransportRequest};
use crate::wire::MappedEvent;

/// Adapter for Google Gemini models.
pub struct GoogleAdapter(SseProvider);

impl GoogleAdapter {
    /// Serves `model` at the public Gemini endpoint.
    pub fn new(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
    ) -> Self {
        Self::at_base(
            transport,
            auth,
            model,
            "https://generativelanguage.googleapis.com",
        )
    }

    /// Custom base (Vertex-style gateways that proxy the same shape).
    #[must_use]
    pub fn at_base(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
        base_url: &str,
    ) -> Self {
        let identity = ModelIdentity::new("google", model);
        let codec = GeminiCodec {
            url: format!(
                "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
                base_url.trim_end_matches('/'),
                model
            ),
        };
        Self(SseProvider::new(transport, auth, Box::new(codec), identity))
    }

    /// The billing surface for the cost HUD.
    #[must_use]
    pub fn auth_kind(&self) -> AuthKind {
        self.0.auth_kind()
    }
}

#[async_trait]
impl Provider for GoogleAdapter {
    fn identity(&self) -> &ModelIdentity {
        self.0.identity()
    }

    async fn stream_completion(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderEventStream, ProviderStreamError> {
        self.0.stream_completion(request).await
    }
}

pub(crate) struct GeminiCodec {
    url: String,
}

impl WireCodec for GeminiCodec {
    fn fresh(&self) -> Box<dyn StreamCodec> {
        Box::new(GeminiStream::default())
    }

    fn render_request(&self, request: &CompletionRequest, auth: &AccessToken) -> TransportRequest {
        let mut body = json!({
            "contents": [{ "role": "user", "parts": super::wire_media::google_parts(request) }],
        });
        if let Some(system) = super::wire_media::google_system(request) {
            body["systemInstruction"] = system;
        }
        if let Some(tools) = super::wire_media::google_tools(request) {
            body["tools"] = tools;
        }
        json_request(
            &self.url,
            &body,
            auth,
            &[
                ("content-type", "application/json"),
                ("accept", "text/event-stream"),
            ],
        )
    }
}

#[derive(Default)]
pub(crate) struct GeminiStream {
    call_ordinal: u64,
    saw_function_call: bool,
    served: Option<String>,
    pending_finish: Option<FinishReason>,
}

impl GeminiStream {
    fn origin(&self, configured: &ModelIdentity) -> ModelIdentity {
        stamp(configured, self.served.as_deref())
    }
}

impl StreamCodec for GeminiStream {
    fn feed(&mut self, frame: &SseFrame, configured: &ModelIdentity) -> Vec<MappedEvent> {
        if let Some(model) = frame.data.get("modelVersion").and_then(Value::as_str) {
            self.served = Some(model.to_owned());
        }
        let origin = self.origin(configured);
        let mut out = Vec::new();
        if let Some(usage) = frame.data.get("usageMetadata") {
            out.push(MappedEvent::Event(ProviderEvent::Usage {
                model: origin.clone(),
                usage: Usage::with_cache(
                    usage
                        .get("promptTokenCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    usage
                        .get("candidatesTokenCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    usage
                        .get("cachedContentTokenCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    0,
                ),
            }));
        }
        let candidate = frame
            .data
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|candidates| candidates.first())
            .unwrap_or(&Value::Null);
        for part in candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                out.push(MappedEvent::Event(ProviderEvent::TextDelta {
                    model: origin.clone(),
                    text: text.to_owned(),
                }));
            }
            if let Some(call) = part.get("functionCall") {
                let name = call.get("name").and_then(Value::as_str).unwrap_or_default();
                let call_id = format!("gemini-call-{}", self.call_ordinal);
                self.call_ordinal += 1;
                self.saw_function_call = true;
                let arguments = call.get("args").cloned().unwrap_or(json!({}));
                out.push(MappedEvent::Event(ProviderEvent::ToolCallStart {
                    model: origin.clone(),
                    call_id: call_id.clone(),
                    name: name.to_owned(),
                }));
                out.push(MappedEvent::Event(ProviderEvent::ToolCallDelta {
                    model: origin.clone(),
                    call_id,
                    arguments_fragment: serde_json::to_string(&arguments)
                        .unwrap_or_else(|_| "{}".to_owned()),
                }));
            }
        }
        if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
            let mapped = finish_reason(reason, self.saw_function_call).unwrap_or_else(|| {
                tracing::warn!(reason, "unknown Gemini finishReason; mapping to EndTurn");
                FinishReason::EndTurn
            });
            self.pending_finish = Some(mapped);
        }
        if out.is_empty() {
            out.push(MappedEvent::Skipped);
        }
        out
    }

    fn flush(&mut self, configured: &ModelIdentity) -> Vec<MappedEvent> {
        match self.pending_finish.take() {
            Some(reason) => vec![MappedEvent::Event(ProviderEvent::Finish {
                model: self.origin(configured),
                reason,
            })],
            None => Vec::new(),
        }
    }
}

fn finish_reason(wire: &str, saw_function_call: bool) -> Option<FinishReason> {
    match wire {
        "STOP" => Some(if saw_function_call {
            FinishReason::ToolCall
        } else {
            FinishReason::EndTurn
        }),
        "MAX_TOKENS" => Some(FinishReason::Length),
        "SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "RECITATION" => Some(FinishReason::Refusal),
        _ => None,
    }
}
