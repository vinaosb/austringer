//! Anthropic Messages API adapter (`POST /v1/messages`, `stream: true`).

use std::collections::BTreeMap;
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
use crate::transport::Transport;
use crate::wire::MappedEvent;

/// Adapter for the Anthropic Messages API.
///
/// Wire mapping: `content_block_delta.text_delta` → text;
/// `tool_use` blocks (id + `input_json_delta` fragments) → tool calls with
/// the provider's stable ids; `message_start`/`message_delta` usage merges
/// into one cache-aware [`Usage`] event; `stop_reason` maps
/// end_turn/stop_sequence→EndTurn, max_tokens→Length, tool_use→ToolCall,
/// refusal→Refusal. `ping`/`message_stop` are protocol noise.
pub struct AnthropicAdapter(SseProvider);

impl AnthropicAdapter {
    /// Serves `model` at the public Anthropic endpoint with the default
    /// output ceiling (4096 tokens — the API requires `max_tokens`).
    pub fn new(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
    ) -> Self {
        Self::custom(transport, auth, model, "https://api.anthropic.com", 4096)
    }

    /// Fully-parameterized constructor (self-hosted gateways, custom
    /// ceilings).
    #[must_use]
    pub fn custom(
        transport: Arc<dyn Transport>,
        auth: Arc<dyn CredentialSource>,
        model: &str,
        base_url: &str,
        max_tokens: u32,
    ) -> Self {
        let identity = ModelIdentity::new("anthropic", model);
        let codec = AnthropicCodec {
            url: format!("{}/v1/messages", base_url.trim_end_matches('/')),
            max_tokens,
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
impl Provider for AnthropicAdapter {
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

pub(crate) struct AnthropicCodec {
    url: String,
    max_tokens: u32,
}

impl WireCodec for AnthropicCodec {
    fn fresh(&self) -> Box<dyn StreamCodec> {
        Box::new(AnthropicStream::default())
    }

    fn render_request(
        &self,
        request: &CompletionRequest,
        auth: &AccessToken,
    ) -> crate::transport::TransportRequest {
        let body = json!({
            "model": request.identity.model,
            "max_tokens": self.max_tokens,
            "stream": true,
            "messages": [{ "role": "user", "content": request.prompt }],
        });
        json_request(
            &self.url,
            &body,
            auth,
            &[
                ("content-type", "application/json"),
                ("accept", "text/event-stream"),
                ("anthropic-version", "2023-06-01"),
            ],
        )
    }
}

#[derive(Default)]
pub(crate) struct AnthropicStream {
    tool_blocks: BTreeMap<u64, String>,
    input_side: Option<(u64, u64, u64)>,
    served: Option<String>,
}

impl AnthropicStream {
    fn origin(&self, configured: &ModelIdentity) -> ModelIdentity {
        stamp(configured, self.served.as_deref())
    }

    fn skip_logged(name: &str, why: &str) -> Vec<MappedEvent> {
        tracing::warn!(event = name, reason = why, "skipping unmappable frame");
        vec![MappedEvent::Skipped]
    }
}

impl StreamCodec for AnthropicStream {
    fn feed(&mut self, frame: &SseFrame, configured: &ModelIdentity) -> Vec<MappedEvent> {
        let name = frame.event.as_deref().unwrap_or_default();
        let origin = self.origin(configured);
        match name {
            "message_start" => {
                let message = frame.data.get("message").unwrap_or(&Value::Null);
                if let Some(model) = message.get("model").and_then(Value::as_str) {
                    self.served = Some(model.to_owned());
                }
                if let Some(usage) = message.get("usage") {
                    self.input_side = Some((
                        field(usage, "input_tokens"),
                        field(usage, "cache_read_input_tokens"),
                        field(usage, "cache_creation_input_tokens"),
                    ));
                }
                vec![MappedEvent::Skipped]
            }
            "content_block_start" => {
                let block = frame.data.get("content_block").unwrap_or(&Value::Null);
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    match (
                        block.get("id").and_then(Value::as_str),
                        block.get("name").and_then(Value::as_str),
                        frame.data.get("index").and_then(Value::as_u64),
                    ) {
                        (Some(id), Some(tool), Some(index)) => {
                            self.tool_blocks.insert(index, id.to_owned());
                            vec![MappedEvent::Event(ProviderEvent::ToolCallStart {
                                model: origin,
                                call_id: id.to_owned(),
                                name: tool.to_owned(),
                            })]
                        }
                        _ => Self::skip_logged(name, "tool_use block missing id/name/index"),
                    }
                } else {
                    vec![MappedEvent::Skipped]
                }
            }
            "content_block_delta" => {
                let delta = frame.data.get("delta").unwrap_or(&Value::Null);
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => match delta.get("text").and_then(Value::as_str) {
                        Some(text) => vec![MappedEvent::Event(ProviderEvent::TextDelta {
                            model: origin,
                            text: text.to_owned(),
                        })],
                        None => Self::skip_logged(name, "text_delta without text"),
                    },
                    Some("input_json_delta") => {
                        let index = frame.data.get("index").and_then(Value::as_u64);
                        let fragment = delta.get("partial_json").and_then(Value::as_str);
                        match (self.tool_blocks.get(&index.unwrap_or(u64::MAX)), fragment) {
                            (Some(call_id), Some(fragment)) => {
                                vec![MappedEvent::Event(ProviderEvent::ToolCallDelta {
                                    model: origin,
                                    call_id: call_id.clone(),
                                    arguments_fragment: fragment.to_owned(),
                                })]
                            }
                            _ => Self::skip_logged(name, "input_json_delta without block/payload"),
                        }
                    }
                    _ => Self::skip_logged(name, "unknown delta type"),
                }
            }
            "message_delta" => {
                let mut out = Vec::new();
                let usage = frame.data.get("usage");
                let (start_input, start_read, start_write) = self.input_side.unwrap_or((0, 0, 0));
                if let Some(usage) = usage {
                    out.push(MappedEvent::Event(ProviderEvent::Usage {
                        model: origin.clone(),
                        usage: Usage::with_cache(
                            usage_field(usage, "input_tokens", start_input),
                            usage_field(usage, "output_tokens", 0),
                            usage_field(usage, "cache_read_input_tokens", start_read),
                            usage_field(usage, "cache_creation_input_tokens", start_write),
                        ),
                    }));
                }
                let reason = frame
                    .data
                    .get("delta")
                    .and_then(|delta| delta.get("stop_reason"))
                    .and_then(Value::as_str);
                if let Some(reason) = reason.and_then(stop_reason) {
                    out.push(MappedEvent::Event(ProviderEvent::Finish {
                        model: origin,
                        reason,
                    }));
                }
                out
            }
            "message_stop" | "ping" => vec![MappedEvent::Skipped],
            "error" => {
                let error = frame.data.get("error").unwrap_or(&Value::Null);
                vec![MappedEvent::Error(ProviderStreamError::StreamError {
                    code: error
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_owned(),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("no message")
                        .to_owned(),
                })]
            }
            _ => Self::skip_logged(name, "unknown event name"),
        }
    }

    fn flush(&mut self, _configured: &ModelIdentity) -> Vec<MappedEvent> {
        Vec::new()
    }
}

fn field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn usage_field(usage: &Value, key: &str, fallback: u64) -> u64 {
    usage.get(key).and_then(Value::as_u64).unwrap_or(fallback)
}

fn stop_reason(wire: &str) -> Option<FinishReason> {
    match wire {
        "end_turn" | "stop_sequence" => Some(FinishReason::EndTurn),
        "max_tokens" => Some(FinishReason::Length),
        "tool_use" => Some(FinishReason::ToolCall),
        "refusal" => Some(FinishReason::Refusal),
        _ => None,
    }
}
