//! Wire mapping: SSE event names + JSON payloads to typed [`ProviderEvent`]s.
//!
//! Malformed or unknown events are SKIPPED and logged — one bad payload must
//! not fail the turn (the T2 parser already tolerates torn frames; this layer
//! finishes the job at the payload level). Identity stamping: events carry
//! the configured identity, except when the wire reports the model it
//! actually served (`data.model`), which wins — that is what makes silent
//! substitution detectable by the turn layer.

use serde_json::Value;

use crate::error::ProviderStreamError;
use crate::event::{FinishReason, ProviderEvent, Usage};
use crate::identity::ModelIdentity;

/// Outcome of mapping one wire event.
#[derive(Debug, Clone, PartialEq)]
pub enum MappedEvent {
    /// A typed provider event stamped with its origin identity.
    Event(ProviderEvent),
    /// Unmappable: skipped and logged; the stream continues.
    Skipped,
    /// A typed in-stream error surfaced by the provider.
    Error(ProviderStreamError),
}

/// Maps one parsed SSE event (`event:` name + `data:` JSON) to zero, one, or
/// two typed outcomes. Complete `tool_call` events expand to a
/// `ToolCallStart` plus one full-arguments `ToolCallDelta` so downstream
/// assembly has a single shape.
pub fn map_wire_event(name: &str, data: &Value, configured: &ModelIdentity) -> Vec<MappedEvent> {
    let origin = stamped_identity(data, configured);
    match name {
        "message_start" | "done" => vec![MappedEvent::Skipped],
        "text_delta" => match str_field(data, "text") {
            Some(text) => vec![event(ProviderEvent::TextDelta {
                model: origin,
                text: text.to_owned(),
            })],
            None => skip(name, "missing or non-string text"),
        },
        "tool_call_start" => match (str_field(data, "call_id"), str_field(data, "name")) {
            (Some(call_id), Some(tool)) => vec![event(ProviderEvent::ToolCallStart {
                model: origin,
                call_id: call_id.to_owned(),
                name: tool.to_owned(),
            })],
            _ => skip(name, "missing call_id or name"),
        },
        "tool_call_delta" => match (str_field(data, "call_id"), str_field(data, "arguments")) {
            (Some(call_id), Some(fragment)) => {
                vec![event(ProviderEvent::ToolCallDelta {
                    model: origin,
                    call_id: call_id.to_owned(),
                    arguments_fragment: fragment.to_owned(),
                })]
            }
            _ => skip(name, "missing call_id or arguments"),
        },
        "tool_call" => match (
            str_field(data, "call_id"),
            str_field(data, "name"),
            str_field(data, "arguments"),
        ) {
            (Some(call_id), Some(tool), Some(args)) => vec![
                event(ProviderEvent::ToolCallStart {
                    model: origin.clone(),
                    call_id: call_id.to_owned(),
                    name: tool.to_owned(),
                }),
                event(ProviderEvent::ToolCallDelta {
                    model: origin,
                    call_id: call_id.to_owned(),
                    arguments_fragment: args.to_owned(),
                }),
            ],
            _ => skip(name, "missing call_id, name, or arguments"),
        },
        "usage" => vec![event(ProviderEvent::Usage {
            model: origin,
            usage: Usage {
                input_tokens: u64_field(data, "input_tokens"),
                output_tokens: u64_field(data, "output_tokens"),
                cache_read_tokens: u64_field(data, "cache_read_tokens"),
                cache_write_tokens: u64_field(data, "cache_write_tokens"),
            },
        })],
        "message_stop" => vec![event(ProviderEvent::Finish {
            model: origin,
            reason: FinishReason::EndTurn,
        })],
        "finish" => match str_field(data, "reason").and_then(finish_reason) {
            Some(reason) => vec![event(ProviderEvent::Finish {
                model: origin,
                reason,
            })],
            None => skip(name, "unknown finish reason"),
        },
        "error" => vec![MappedEvent::Error(ProviderStreamError::StreamError {
            code: str_field(data, "code").unwrap_or("unknown").to_owned(),
            message: str_field(data, "message")
                .unwrap_or("no message")
                .to_owned(),
        })],
        _ => skip(name, "unknown event name"),
    }
}

fn event(ev: ProviderEvent) -> MappedEvent {
    MappedEvent::Event(ev)
}

fn skip(name: &str, why: &str) -> Vec<MappedEvent> {
    tracing::warn!(
        event = name,
        reason = why,
        "skipping unmappable provider event"
    );
    vec![MappedEvent::Skipped]
}

fn str_field<'a>(data: &'a Value, key: &str) -> Option<&'a str> {
    data.get(key).and_then(Value::as_str)
}

fn u64_field(data: &Value, key: &str) -> u64 {
    data.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn finish_reason(wire: &str) -> Option<FinishReason> {
    match wire {
        "end_turn" => Some(FinishReason::EndTurn),
        "length" => Some(FinishReason::Length),
        "tool_call" => Some(FinishReason::ToolCall),
        "refusal" => Some(FinishReason::Refusal),
        _ => None,
    }
}

fn stamped_identity(data: &Value, configured: &ModelIdentity) -> ModelIdentity {
    match str_field(data, "model") {
        Some(served) if served != configured.model => ModelIdentity {
            provider: configured.provider.clone(),
            model: served.to_owned(),
        },
        _ => configured.clone(),
    }
}
