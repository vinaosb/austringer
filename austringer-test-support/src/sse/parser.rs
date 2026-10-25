//! Consumer-side SSE parser that exposes partial-frame state, so tests can
//! observe exactly where a dropped stream tore mid-event.

use serde_json::Value;

use super::event::SseEvent;

/// Incremental parser over raw SSE bytes.
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    events: Vec<SseEvent>,
}

impl SseParser {
    /// Creates an empty parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends raw bytes and extracts every complete `\n\n`-terminated frame.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
        while let Some(pos) = self.buffer.windows(2).position(|w| w == b"\n\n") {
            let block: Vec<u8> = self.buffer.drain(..pos + 2).collect();
            if let Some(ev) = parse_block(&block[..pos]) {
                self.events.push(ev);
            }
        }
    }

    /// Every complete event parsed so far, in order.
    pub fn events(&self) -> &[SseEvent] {
        &self.events
    }

    /// Bytes of the trailing incomplete frame (empty when the stream ended on a
    /// frame boundary).
    pub fn pending_fragment(&self) -> &[u8] {
        &self.buffer
    }

    /// True when the last fed bytes ended in the middle of an event.
    pub fn is_mid_event(&self) -> bool {
        !self.buffer.is_empty()
    }
}

/// Parses one `event:`/`data:` block (without its blank-line terminator).
/// Returns `None` for blocks without a valid `event:` line or non-UTF-8 data;
/// unparsable `data:` JSON degrades to a string payload instead of failing.
fn parse_block(block: &[u8]) -> Option<SseEvent> {
    let text = std::str::from_utf8(block).ok()?;
    let mut name: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("event:") {
            name = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim());
        }
    }
    let data_raw = data_lines.join("\n");
    let data = serde_json::from_str::<Value>(&data_raw)
        .unwrap_or_else(|_| Value::String(data_raw.clone()));
    Some(SseEvent::new(&name?, data))
}
