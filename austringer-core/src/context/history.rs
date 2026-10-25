//! Deterministic rendering of T4 history events into the `[L1,L2)`
//! history segment (plan todo 11).
//!
//! Only LLM-visible events render (internal condensation/snapshot markers
//! never enter model context — the visibility split is a property of the
//! event type). Each event renders as one `[seq] kind: payload` line in
//! the given order: the builder never reorders history. Image blocks
//! render as their typed placeholder markers; tool-call arguments
//! serialize with sorted keys, so the same event list always yields the
//! same bytes.

use crate::event::{ContentBlock, Event, RecordedEvent};

/// Renders the LLM-visible events as newline-joined history lines.
/// Internal events are skipped by visibility class, not by name.
pub(crate) fn render_history(events: &[RecordedEvent]) -> String {
    let mut lines = Vec::with_capacity(events.len());
    for recorded in events {
        let seq = recorded.seq.0;
        let line = match &recorded.event {
            Event::User { blocks } => {
                format!("[{seq}] user: {}", markers(blocks))
            }
            Event::Assistant { blocks } => {
                format!("[{seq}] assistant: {}", markers(blocks))
            }
            Event::ToolCall {
                call_id,
                tool,
                arguments,
            } => format!(
                "[{seq}] tool-call {call_id} {tool} {}",
                // Infallible for serde_json values (sorted-key compact form).
                serde_json::to_string(arguments).unwrap_or_default()
            ),
            Event::ToolResult { call_id, blocks } => {
                format!("[{seq}] tool-result {call_id} {}", markers(blocks))
            }
            Event::System { text } => {
                format!("[{seq}] system: {text}")
            }
            Event::Condensation { .. } | Event::Snapshot { .. } => continue,
        };
        lines.push(line);
    }
    lines.join("\n")
}

fn markers(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .map(ContentBlock::text_marker)
        .collect::<Vec<_>>()
        .join("\n")
}
