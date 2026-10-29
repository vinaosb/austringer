//! The chat-completions wire family: OpenAI `/v1/chat/completions`, xAI,
//! and every OpenAI-compatible endpoint. Frames are data-only (no `event:`
//! line); the codec dispatches purely on the JSON payload, which also makes
//! it format-tolerant of proxies that add event names.
//!
//! Tool calls arrive as `delta.tool_calls[]` entries keyed by `index`: the
//! first fragment carries `id` + `function.name`; later fragments carry
//! argument fragments referencing the same index. The index→id map is
//! per-stream state.
//!
//! `finish_reason` arrives BEFORE the trailing usage chunk, so `Finish` is
//! buffered and flushed at end-of-stream — the consumer always sees usage
//! then finish, never finish-then-usage truncation.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{StreamCodec, WireCodec, json_request, stamp};
use crate::auth::AccessToken;
use crate::event::{FinishReason, ProviderEvent, Usage};
use crate::identity::{CompletionRequest, ModelIdentity};
use crate::sse::SseFrame;
use crate::transport::TransportRequest;
use crate::wire::MappedEvent;

/// Shared codec for the chat-completions wire family.
pub(crate) struct ChatCompletionsCodec {
    pub(crate) url: String,
}

impl WireCodec for ChatCompletionsCodec {
    fn fresh(&self) -> Box<dyn StreamCodec> {
        Box::new(ChatCompletionsStream::default())
    }

    fn render_request(&self, request: &CompletionRequest, auth: &AccessToken) -> TransportRequest {
        let mut body = json!({
            "model": request.identity.model,
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": super::wire_media::chat_messages(request),
        });
        if let Some(tools) = super::wire_media::chat_tools(request) {
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
pub(crate) struct ChatCompletionsStream {
    tools: BTreeMap<u64, String>,
    served: Option<String>,
    pending_finish: Option<FinishReason>,
}

impl ChatCompletionsStream {
    fn origin(&self, configured: &ModelIdentity) -> ModelIdentity {
        stamp(configured, self.served.as_deref())
    }

    fn skip_logged(why: &str) -> Vec<MappedEvent> {
        tracing::warn!(reason = why, "skipping unmappable chat-completions frame");
        vec![MappedEvent::Skipped]
    }

    fn feed_delta(&mut self, delta: &Value, origin: &ModelIdentity) -> Vec<MappedEvent> {
        let mut out = Vec::new();
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            out.push(MappedEvent::Event(ProviderEvent::TextDelta {
                model: origin.clone(),
                text: text.to_owned(),
            }));
        }
        for call in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let index = call
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX);
            let id = call.get("id").and_then(Value::as_str);
            let function = call.get("function").unwrap_or(&Value::Null);
            let name = function.get("name").and_then(Value::as_str);
            let arguments = function.get("arguments").and_then(Value::as_str);
            if let (Some(id), Some(name)) = (id, name) {
                self.tools.insert(index, id.to_owned());
                out.push(MappedEvent::Event(ProviderEvent::ToolCallStart {
                    model: origin.clone(),
                    call_id: id.to_owned(),
                    name: name.to_owned(),
                }));
            }
            if let Some(fragment) = arguments {
                let call_id = id
                    .map(str::to_owned)
                    .or_else(|| self.tools.get(&index).cloned());
                match call_id {
                    Some(call_id) => {
                        out.push(MappedEvent::Event(ProviderEvent::ToolCallDelta {
                            model: origin.clone(),
                            call_id,
                            arguments_fragment: fragment.to_owned(),
                        }));
                    }
                    None => {
                        out.extend(Self::skip_logged("tool fragment without known call id"));
                    }
                }
            }
        }
        out
    }
}

impl StreamCodec for ChatCompletionsStream {
    fn feed(&mut self, frame: &SseFrame, configured: &ModelIdentity) -> Vec<MappedEvent> {
        if let Some(model) = frame.data.get("model").and_then(Value::as_str) {
            self.served = Some(model.to_owned());
        }
        if frame.data == Value::String("[DONE]".to_owned()) {
            return vec![MappedEvent::Skipped];
        }
        let origin = self.origin(configured);
        let mut out = Vec::new();
        if let Some(usage) = frame.data.get("usage").filter(|usage| !usage.is_null()) {
            out.push(MappedEvent::Event(ProviderEvent::Usage {
                model: origin.clone(),
                usage: Usage::with_cache(
                    usage
                        .get("prompt_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    usage
                        .get("completion_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    usage
                        .get("prompt_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    0,
                ),
            }));
        }
        let choice = frame
            .data
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .unwrap_or(&Value::Null);
        if let Some(delta) = choice.get("delta").filter(|delta| !delta.is_null()) {
            out.extend(self.feed_delta(delta, &origin));
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            match finish_reason(reason) {
                Some(reason) => self.pending_finish = Some(reason),
                None => out.extend(Self::skip_logged("unknown finish_reason")),
            }
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

fn finish_reason(wire: &str) -> Option<FinishReason> {
    match wire {
        "stop" => Some(FinishReason::EndTurn),
        "length" => Some(FinishReason::Length),
        "tool_calls" => Some(FinishReason::ToolCall),
        "content_filter" => Some(FinishReason::Refusal),
        _ => None,
    }
}
