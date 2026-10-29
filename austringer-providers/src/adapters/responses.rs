//! OpenAI Responses API codec: `response.output_text.delta`,
//! `response.output_item.added` (function_call items),
//! `response.function_call_arguments.delta`, `response.completed` /
//! `response.incomplete` / `response.failed`.
//!
//! Like the chat-completions family, `Finish` is buffered and flushed at
//! end-of-stream: `response.completed` arrives before terminal sub-events
//! some proxies split off, and the consumer must see usage before finish.

use serde_json::{Value, json};

use super::{StreamCodec, WireCodec};
use crate::auth::AccessToken;
use crate::error::ProviderStreamError;
use crate::event::{FinishReason, ProviderEvent, Usage};
use crate::identity::{CompletionRequest, ModelIdentity};
use crate::sse::SseFrame;
use crate::transport::TransportRequest;
use crate::wire::MappedEvent;

/// Codec for the Responses API wire.
pub(crate) struct ResponsesCodec {
    pub(crate) url: String,
}

impl WireCodec for ResponsesCodec {
    fn fresh(&self) -> Box<dyn StreamCodec> {
        Box::new(ResponsesStream::default())
    }

    fn render_request(&self, request: &CompletionRequest, auth: &AccessToken) -> TransportRequest {
        let mut body = json!({
            "model": request.identity.model,
            "stream": true,
            "input": super::wire_media::responses_input(request),
        });
        if let Some(instructions) = super::wire_media::responses_instructions(request) {
            body["instructions"] = json!(instructions);
        }
        if let Some(tools) = super::wire_media::responses_tools(request) {
            body["tools"] = tools;
        }
        super::json_request(
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
pub(crate) struct ResponsesStream {
    saw_function_call: bool,
    served: Option<String>,
    pending_finish: Option<FinishReason>,
}

impl ResponsesStream {
    fn origin(&self, configured: &ModelIdentity) -> ModelIdentity {
        super::stamp(configured, self.served.as_deref())
    }
}

impl StreamCodec for ResponsesStream {
    fn feed(&mut self, frame: &SseFrame, configured: &ModelIdentity) -> Vec<MappedEvent> {
        let name = frame.event.as_deref().unwrap_or_default();
        let origin = self.origin(configured);
        match name {
            "response.output_text.delta" => match frame.data.get("delta").and_then(Value::as_str) {
                Some(text) => vec![MappedEvent::Event(ProviderEvent::TextDelta {
                    model: origin,
                    text: text.to_owned(),
                })],
                None => vec![MappedEvent::Skipped],
            },
            "response.output_item.added" => {
                let item = frame.data.get("item").unwrap_or(&Value::Null);
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    match (
                        item.get("id").and_then(Value::as_str),
                        item.get("name").and_then(Value::as_str),
                    ) {
                        (Some(id), Some(tool)) => {
                            self.saw_function_call = true;
                            vec![MappedEvent::Event(ProviderEvent::ToolCallStart {
                                model: origin,
                                call_id: id.to_owned(),
                                name: tool.to_owned(),
                            })]
                        }
                        _ => vec![MappedEvent::Skipped],
                    }
                } else {
                    vec![MappedEvent::Skipped]
                }
            }
            "response.function_call_arguments.delta" => {
                match (
                    frame.data.get("item_id").and_then(Value::as_str),
                    frame.data.get("delta").and_then(Value::as_str),
                ) {
                    (Some(call_id), Some(fragment)) => {
                        vec![MappedEvent::Event(ProviderEvent::ToolCallDelta {
                            model: origin,
                            call_id: call_id.to_owned(),
                            arguments_fragment: fragment.to_owned(),
                        })]
                    }
                    _ => vec![MappedEvent::Skipped],
                }
            }
            "response.completed" | "response.incomplete" => {
                let response = frame.data.get("response").unwrap_or(&Value::Null);
                if let Some(model) = response.get("model").and_then(Value::as_str) {
                    self.served = Some(model.to_owned());
                }
                let mut out = Vec::new();
                if let Some(usage) = response.get("usage") {
                    out.push(MappedEvent::Event(ProviderEvent::Usage {
                        model: self.origin(configured),
                        usage: Usage::with_cache(
                            usage
                                .get("input_tokens")
                                .and_then(Value::as_u64)
                                .unwrap_or(0),
                            usage
                                .get("output_tokens")
                                .and_then(Value::as_u64)
                                .unwrap_or(0),
                            usage
                                .get("input_tokens_details")
                                .and_then(|details| details.get("cached_tokens"))
                                .and_then(Value::as_u64)
                                .unwrap_or(0),
                            0,
                        ),
                    }));
                }
                let reason = if name == "response.incomplete" {
                    let cause = response
                        .get("incomplete_details")
                        .and_then(|details| details.get("reason"))
                        .and_then(Value::as_str)
                        .unwrap_or("max_output_tokens");
                    if cause == "content_filter" {
                        FinishReason::Refusal
                    } else {
                        FinishReason::Length
                    }
                } else if self.saw_function_call {
                    FinishReason::ToolCall
                } else {
                    FinishReason::EndTurn
                };
                self.pending_finish = Some(reason);
                out
            }
            "response.failed" => {
                let response = frame.data.get("response").unwrap_or(&Value::Null);
                let error = response.get("error").unwrap_or(&Value::Null);
                vec![MappedEvent::Error(ProviderStreamError::StreamError {
                    code: error
                        .get("code")
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
            _ => vec![MappedEvent::Skipped],
        }
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
