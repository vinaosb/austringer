//! Codex session reader (`~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`).
//!
//! Mapping:
//! * `response_item`/`message` (role `user`) → [`Event::User`]
//! * `response_item`/`message` (role `assistant`) → [`Event::Assistant`]
//! * `response_item`/`function_call` → [`Event::ToolCall`] (arguments
//!   arrive as a JSON *string*; parsed when possible, kept verbatim
//!   otherwise — disclosed)
//! * `response_item`/`function_call_output` → [`Event::ToolResult`]
//! * `event_msg`/`agent_message` → [`Event::Assistant`]
//! * everything else (`session_meta`, `reasoning`, `compacted`, local
//!   shell calls, malformed lines) → [`UnknownForeignEntry`].

use serde_json::Value;

use austringer_core::event::{ContentBlock, Event};

use super::{ForeignRead, ForeignReason, UnknownForeignEntry};

/// Reads one Codex rollout JSONL file.
#[must_use]
pub fn read(raw: &[u8]) -> ForeignRead {
    let mut read = ForeignRead::default();
    read.torn_tail = super::read_lines(raw, |line_no, line| {
        let value: Value = match serde_json::from_slice(line) {
            Ok(value) => value,
            Err(_) => {
                read.unknown.push(UnknownForeignEntry {
                    line: line_no,
                    kind: "malformed".to_owned(),
                    reason: ForeignReason::MalformedJson,
                });
                return;
            }
        };
        let kind = format!(
            "{}/{}",
            value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("untyped"),
            value
                .pointer("/payload/type")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        match map_line(&value) {
            Ok(Some(events)) => read.events.extend(events),
            Ok(None) => read.unknown.push(UnknownForeignEntry {
                line: line_no,
                kind,
                reason: ForeignReason::UnmappedType,
            }),
            Err(reason) => read.unknown.push(UnknownForeignEntry {
                line: line_no,
                kind,
                reason,
            }),
        }
    });
    read
}

type MapResult = Result<Option<Vec<Event>>, ForeignReason>;

fn map_line(value: &Value) -> MapResult {
    let line_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let payload = value.get("payload").unwrap_or(&Value::Null);
    let payload_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
    match (line_type, payload_type) {
        ("response_item", "message") => map_message(payload),
        ("response_item", "function_call") => Ok(Some(vec![Event::ToolCall {
            call_id: payload
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            tool: payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            arguments: parse_arguments(payload.get("arguments")),
        }])),
        ("response_item", "function_call_output") => Ok(Some(vec![Event::ToolResult {
            call_id: payload
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            blocks: vec![ContentBlock::Text {
                text: payload
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }],
        }])),
        ("event_msg", "agent_message") => Ok(Some(vec![Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: payload
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }],
        }])),
        _ => Ok(None),
    }
}

fn map_message(payload: &Value) -> MapResult {
    let role = payload.get("role").and_then(Value::as_str).unwrap_or("");
    let content = payload
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| ForeignReason::WrongShape {
            detail: "message without content array".to_owned(),
        })?;
    let mut blocks = Vec::new();
    for part in content {
        match part.get("type").and_then(Value::as_str).unwrap_or("") {
            "input_text" | "output_text" | "text" => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    blocks.push(ContentBlock::Text {
                        text: text.to_owned(),
                    });
                }
            }
            other => {
                return Err(ForeignReason::UnsupportedShape {
                    detail: format!("content part {other:?} has no T4 counterpart"),
                });
            }
        }
    }
    match role {
        "user" => Ok(Some(vec![Event::User { blocks }])),
        "assistant" => Ok(Some(vec![Event::Assistant { blocks }])),
        _ => Ok(None),
    }
}

fn parse_arguments(raw: Option<&Value>) -> Value {
    match raw {
        Some(Value::String(text)) => {
            serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone()))
        }
        Some(value) => value.clone(),
        None => Value::Null,
    }
}
