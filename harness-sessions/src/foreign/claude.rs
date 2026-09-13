//! Claude Code session reader (`~/.claude/projects/**/**.jsonl`).
//!
//! Mapping (lossless where the formats align):
//! * `type: "user"` with string content → [`Event::User`]
//! * `type: "user"` with `tool_result` blocks → [`Event::ToolResult`]
//! * `type: "assistant"` → text blocks join into [`Event::Assistant`];
//!   each `tool_use` block becomes its own [`Event::ToolCall`]
//! * `type: "system"` with string `content` → [`Event::System`]
//! * everything else (`summary`, `file-history-snapshot`, malformed
//!   lines, inline image blocks that violate T4's out-of-line media
//!   rule) → [`UnknownForeignEntry`], disclosed not dropped.

use serde_json::Value;

use harness_core::event::{ContentBlock, Event};

use super::{ForeignRead, ForeignReason, UnknownForeignEntry};

/// Reads one Claude Code session JSONL file.
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
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("untyped")
            .to_owned();
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
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "user" => map_user(value),
        "assistant" => map_assistant(value),
        "system" => match value.get("content") {
            Some(Value::String(text)) => Ok(Some(vec![Event::System { text: text.clone() }])),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn text_block(value: &Value) -> Option<ContentBlock> {
    value
        .get("text")
        .and_then(Value::as_str)
        .map(|text| ContentBlock::Text {
            text: text.to_owned(),
        })
}

fn map_user(value: &Value) -> MapResult {
    let content = value
        .pointer("/message/content")
        .ok_or_else(|| wrong("user entry without message.content".to_owned()))?;
    match content {
        Value::String(text) => Ok(Some(vec![Event::User {
            blocks: vec![ContentBlock::Text { text: text.clone() }],
        }])),
        Value::Array(blocks) => {
            let mut events = Vec::new();
            let mut result_blocks = Vec::new();
            for block in blocks {
                match block.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(block) = text_block(block) {
                            result_blocks.push(block);
                        }
                    }
                    "tool_result" => {
                        let call_id = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let blocks = tool_result_blocks(block)?;
                        events.push(Event::ToolResult { call_id, blocks });
                    }
                    other => {
                        return Err(unsupported(format!(
                            "user block type {other:?} has no T4 counterpart"
                        )));
                    }
                }
            }
            if !result_blocks.is_empty() {
                events.insert(
                    0,
                    Event::User {
                        blocks: result_blocks,
                    },
                );
            }
            Ok((!events.is_empty()).then_some(events))
        }
        _ => Err(wrong("user content is neither string nor array".to_owned())),
    }
}

fn tool_result_blocks(block: &Value) -> Result<Vec<ContentBlock>, ForeignReason> {
    match block.get("content") {
        Some(Value::String(text)) => Ok(vec![ContentBlock::Text { text: text.clone() }]),
        Some(Value::Array(parts)) => {
            let mut blocks = Vec::new();
            for part in parts {
                match part.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(block) = text_block(part) {
                            blocks.push(block);
                        }
                    }
                    other => {
                        return Err(unsupported(format!(
                            "tool_result part {other:?} has no T4 counterpart"
                        )));
                    }
                }
            }
            Ok(blocks)
        }
        _ => Ok(Vec::new()),
    }
}

fn map_assistant(value: &Value) -> MapResult {
    let content = value
        .pointer("/message/content")
        .ok_or_else(|| wrong("assistant entry without message.content".to_owned()))?;
    let Value::Array(blocks) = content else {
        return Err(wrong("assistant content is not a block array".to_owned()));
    };
    let mut events = Vec::new();
    let mut text_blocks = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => {
                if let Some(block) = text_block(block) {
                    text_blocks.push(block);
                }
            }
            "tool_use" => {
                let call_id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let tool = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned();
                let arguments = block.get("input").cloned().unwrap_or(Value::Null);
                events.push(Event::ToolCall {
                    call_id,
                    tool,
                    arguments,
                });
            }
            other => {
                return Err(unsupported(format!(
                    "assistant block type {other:?} has no T4 counterpart"
                )));
            }
        }
    }
    if !text_blocks.is_empty() {
        events.insert(
            0,
            Event::Assistant {
                blocks: text_blocks,
            },
        );
    }
    Ok((!events.is_empty()).then_some(events))
}

fn wrong(detail: String) -> ForeignReason {
    ForeignReason::WrongShape { detail }
}

fn unsupported(detail: String) -> ForeignReason {
    ForeignReason::UnsupportedShape { detail }
}
