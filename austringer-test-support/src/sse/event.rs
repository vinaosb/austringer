//! Server-sent-event type plus `ev_*` constructors (codex `responses.rs` pattern).
//!
//! Rendering is the standard wire format:
//!
//! ```text
//! event: text_delta
//! data: {"text":"hi"}
//!
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One server-sent event: an `event:` name and a JSON `data:` payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SseEvent {
    /// Event name carried on the `event:` line.
    pub event: String,
    /// JSON payload carried on the `data:` line.
    pub data: Value,
}

impl SseEvent {
    /// Creates an event from a name and an arbitrary JSON payload.
    pub fn new(event: &str, data: Value) -> Self {
        Self {
            event: event.to_owned(),
            data,
        }
    }

    /// Renders the standard SSE frame: `event:` + `data:` lines + blank-line
    /// terminator. Compact JSON with sorted object keys (serde_json default),
    /// so identical events always render byte-identically.
    pub fn render(&self) -> String {
        format!("event: {}\ndata: {}\n\n", self.event, self.data)
    }
}

/// `message_start`: opens the assistant message stream.
pub fn ev_message_start() -> SseEvent {
    SseEvent::new(
        "message_start",
        json!({ "role": "assistant", "type": "message_start" }),
    )
}

/// `message_stop`: closes the assistant message.
pub fn ev_message_stop() -> SseEvent {
    SseEvent::new("message_stop", json!({ "type": "message_stop" }))
}

/// `text_delta`: one fragment of assistant text.
pub fn ev_text_delta(text: &str) -> SseEvent {
    SseEvent::new("text_delta", json!({ "text": text, "type": "text_delta" }))
}

/// `tool_call_delta`: one fragment of tool-call arguments.
pub fn ev_tool_call_delta(call_id: &str, args_fragment: &str) -> SseEvent {
    SseEvent::new(
        "tool_call_delta",
        json!({ "arguments": args_fragment, "call_id": call_id, "type": "tool_call_delta" }),
    )
}

/// `usage`: token accounting for the completed response.
pub fn ev_usage(input_tokens: u64, output_tokens: u64) -> SseEvent {
    SseEvent::new(
        "usage",
        json!({ "input_tokens": input_tokens, "output_tokens": output_tokens, "type": "usage" }),
    )
}

/// `error`: a stream-level error payload.
pub fn ev_error(code: &str, message: &str) -> SseEvent {
    SseEvent::new(
        "error",
        json!({ "code": code, "message": message, "type": "error" }),
    )
}

/// `done`: terminal sentinel mirroring OpenAI's `[DONE]`.
pub fn ev_done() -> SseEvent {
    SseEvent::new("done", json!({ "type": "done" }))
}
