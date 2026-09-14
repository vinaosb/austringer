//! Parsing of the CLI `--output-json` NDJSON contract (todo 39, frozen
//! daemon protocol v1 shapes): one JSON object per line — event frames
//! as `{"event": {"id": N, "event": …}}` (Recorded frames unwrap to the
//! core event) and a final `{"type": "turn_result", "ok": …}` line.
//!
//! Normative wire contract (doc-constants; the tests pin them):
//! - assistant text: `event.event.type == "assistant"`,
//!   `blocks[*].type == "text"` → `blocks[*].text`
//! - tool call: `event.event.type == "tool_call"` with `call_id`,
//!   `tool`, `arguments`
//! - turn end: `type == "turn_result"` with `ok`, `outcome`, `turn`

use serde::{Deserialize, Serialize};

/// One observed tool call (arguments kept as raw sorted-key JSON text
/// so scanners never re-serialize).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallObserved {
    /// Correlation id.
    pub call_id: String,
    /// Tool name.
    pub tool: String,
    /// Raw JSON text of the arguments.
    pub arguments_json: String,
}

/// Everything an eval needs from one finished turn's NDJSON stream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TurnStream {
    /// All assistant text blocks, joined with `\n` (in event order).
    pub assistant_text: String,
    /// Tool calls in event order.
    pub tool_calls: Vec<ToolCallObserved>,
    /// Final turn outcome string.
    pub outcome: String,
    /// Whether the outcome counts as success (render::outcome_ok).
    pub outcome_ok: bool,
}

/// Everything stream parsing can fail with. Typed and bounded: the raw
/// line never rides the error beyond a bounded detail.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WireError {
    /// A line is not valid JSON or not an envelope/turn_result object.
    #[error("bad NDJSON line {line_no}: {detail}")]
    BadLine {
        /// 1-based line number.
        line_no: usize,
        /// Bounded detail (never the raw line).
        detail: String,
    },
    /// The stream ended without a `turn_result` line.
    #[error("stream ended without a turn_result line")]
    MissingTurnResult,
}

impl TurnStream {
    /// Parses the full NDJSON output of one headless run. Unknown
    /// daemon-level events pass through silently; a missing final
    /// `turn_result` is a typed error (the stream is machine-written —
    /// fail loud, never guess).
    ///
    /// # Errors
    /// [`WireError::BadLine`] on malformed lines;
    /// [`WireError::MissingTurnResult`] when no turn_result line exists.
    pub fn parse(lines: &[String]) -> Result<Self, WireError> {
        use serde_json::Value;
        let mut stream = Self::default();
        let mut texts: Vec<String> = Vec::new();
        let mut saw_result = false;
        for (index, line) in lines.iter().enumerate() {
            let value: Value =
                serde_json::from_str(line.trim()).map_err(|e| WireError::BadLine {
                    line_no: index + 1,
                    detail: e.to_string().chars().take(200).collect(),
                })?;
            if value.get("type").and_then(Value::as_str) == Some("turn_result") {
                stream.outcome_ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false);
                stream.outcome = value
                    .get("outcome")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                saw_result = true;
                continue;
            }
            let Some(event) = value.get("event").and_then(|e| e.get("event")) else {
                continue;
            };
            match event.get("type").and_then(Value::as_str) {
                Some("assistant") => {
                    if let Some(blocks) = event.get("blocks").and_then(Value::as_array) {
                        for block in blocks {
                            if block.get("type").and_then(Value::as_str) == Some("text")
                                && let Some(text) = block.get("text").and_then(Value::as_str)
                            {
                                texts.push(text.to_owned());
                            }
                        }
                    }
                }
                Some("tool_call") => stream.tool_calls.push(ToolCallObserved {
                    call_id: event
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    tool: event
                        .get("tool")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    arguments_json: event
                        .get("arguments")
                        .map(Value::to_string)
                        .unwrap_or_else(|| "{}".to_owned()),
                }),
                _ => {}
            }
        }
        if !saw_result {
            return Err(WireError::MissingTurnResult);
        }
        stream.assistant_text = texts.join("\n");
        Ok(stream)
    }

    /// The first two tool-call names (the TSS golden prefix, todo 44).
    #[must_use]
    pub fn first_two_tool_calls(&self) -> Vec<String> {
        self.tool_calls
            .iter()
            .take(2)
            .map(|call| call.tool.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant_line(text: &str) -> String {
        serde_json::json!({
            "event": { "id": 1, "event": {
                "type": "assistant",
                "blocks": [ { "type": "text", "text": text } ]
            }}
        })
        .to_string()
    }

    fn tool_line(call_id: &str, tool: &str) -> String {
        serde_json::json!({
            "event": { "id": 2, "event": {
                "type": "tool_call", "call_id": call_id, "tool": tool,
                "arguments": { "path": "a.txt" }
            }}
        })
        .to_string()
    }

    const TURN_OK: &str = r#"{"type":"turn_result","ok":true,"outcome":"task_completed","turn":1}"#;

    #[test]
    fn parses_assistant_text_tool_calls_and_turn_result() {
        let lines = vec![
            assistant_line("partial answer"),
            tool_line("c1", "view_window"),
            assistant_line("final: done"),
            TURN_OK.to_owned(),
        ];
        let stream = TurnStream::parse(&lines).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(stream.assistant_text, "partial answer\nfinal: done");
        assert_eq!(stream.tool_calls.len(), 1);
        assert_eq!(stream.tool_calls[0].tool, "view_window");
        assert!(stream.outcome_ok);
        assert_eq!(stream.outcome, "task_completed");
    }

    #[test]
    fn malformed_line_is_typed_and_missing_result_is_typed() {
        let bad = vec!["{not json".to_owned()];
        match TurnStream::parse(&bad) {
            Err(WireError::BadLine { line_no, .. }) => assert_eq!(line_no, 1),
            other => panic!("expected BadLine, got {other:?}"),
        }
        let no_result = vec![assistant_line("hi")];
        assert_eq!(
            TurnStream::parse(&no_result),
            Err(WireError::MissingTurnResult)
        );
    }

    #[test]
    fn first_two_tool_calls_form_the_golden_prefix() {
        let lines = vec![
            tool_line("c1", "tool_search"),
            tool_line("c2", "view_window"),
            tool_line("c3", "edit_lines"),
            TURN_OK.to_owned(),
        ];
        let stream = TurnStream::parse(&lines).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            stream.first_two_tool_calls(),
            vec!["tool_search".to_owned(), "view_window".to_owned()]
        );
    }

    #[test]
    fn unicode_text_survives_the_roundtrip() {
        let lines = vec![
            assistant_line("回答: 日本語のテキスト 👍🏻"),
            TURN_OK.to_owned(),
        ];
        let stream = TurnStream::parse(&lines).unwrap_or_else(|e| panic!("{e}"));
        assert!(stream.assistant_text.contains("日本語"));
        assert!(stream.assistant_text.contains("\u{1f44d}\u{1f3fb}"));
    }
}
