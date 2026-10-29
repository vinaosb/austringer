//! Production SSE frame parsing for the adapters: byte chunks in,
//! `event:`/`data:` frames out.
//!
//! Lives here (not in austringer-test-support) because test-support is a
//! dev-only crate by design; the adapters need a parser in production. This
//! one hardens the T2 semantics for real wires: CRLF line endings, comment
//! lines, and a bounded frame buffer — provider payloads are untrusted, so
//! the parser never buffers more than [`MAX_FRAME_BYTES`] without a frame
//! boundary.

use std::collections::VecDeque;

use serde_json::Value;

use crate::transport::TransportFailure;

/// Upper bound on the unparsed frame buffer (1 MiB). A stream that sends
/// more than this without a blank-line boundary is a protocol violation,
/// not something to buffer indefinitely.
const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// One parsed SSE frame. `event` is absent for data-only formats (OpenAI
/// chat completions, Gemini `alt=sse`); non-JSON payloads degrade to
/// [`Value::String`] (the OpenAI `[DONE]` sentinel arrives that way).
pub(crate) struct SseFrame {
    /// The `event:` line, when the format carries one.
    pub(crate) event: Option<String>,
    /// The joined `data:` payload.
    pub(crate) data: Value,
}

/// Incremental SSE frame parser over arbitrary chunk boundaries.
#[derive(Default)]
pub(crate) struct SseFrameParser {
    buffer: Vec<u8>,
    frames: VecDeque<SseFrame>,
}

impl SseFrameParser {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Appends raw bytes (CRLF normalized), extracting every complete frame.
    /// Returns [`TransportFailure::Protocol`] once the buffer exceeds
    /// [`MAX_FRAME_BYTES`] without a frame boundary.
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Result<(), TransportFailure> {
        for byte in bytes {
            if *byte != b'\r' {
                self.buffer.push(*byte);
            }
        }
        // Hot path: scan only unconsumed bytes, slice-borrow blocks
        // (parse_block returns an owned frame), one trailing drain.
        let mut consumed = 0usize;
        while let Some(rel) = find_blank_line(&self.buffer[consumed..]) {
            let end = consumed + rel;
            if let Some(frame) = parse_block(&self.buffer[consumed..end]) {
                self.frames.push_back(frame);
            }
            consumed = end + 2;
        }
        if consumed > 0 {
            self.buffer.drain(..consumed);
        }
        if self.buffer.len() > MAX_FRAME_BYTES {
            return Err(TransportFailure::Protocol(format!(
                "no SSE frame boundary within {MAX_FRAME_BYTES} bytes"
            )));
        }
        Ok(())
    }

    /// Pops the next complete frame, if any.
    pub(crate) fn next_frame(&mut self) -> Option<SseFrame> {
        self.frames.pop_front()
    }
}

fn find_blank_line(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|window| window == b"\n\n")
}

/// Parses one frame block (lines before the blank line). Blocks without a
/// `data:` line (comments, event-only keepalives) yield no frame.
fn parse_block(block: &[u8]) -> Option<SseFrame> {
    let text = std::str::from_utf8(block).ok()?;
    let mut event: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(strip_one_space(rest).to_owned());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(strip_one_space(rest));
        }
    }
    if data_lines.is_empty() {
        return None;
    }
    let joined = data_lines.join("\n");
    let data = serde_json::from_str::<Value>(&joined).unwrap_or(Value::String(joined));
    Some(SseFrame { event, data })
}

/// SSE field values may carry exactly one leading space after the colon.
fn strip_one_space(field: &str) -> &str {
    field.strip_prefix(' ').unwrap_or(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames_of(text: &str) -> Vec<SseFrame> {
        let mut parser = SseFrameParser::new();
        parser.feed(text.as_bytes()).expect("parse");
        let mut out = Vec::new();
        while let Some(frame) = parser.next_frame() {
            out.push(frame);
        }
        out
    }

    #[test]
    fn parses_event_and_data_frame() {
        let frames = frames_of("event: text_delta\ndata: {\"text\":\"hi\"}\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event.as_deref(), Some("text_delta"));
        assert_eq!(frames[0].data, serde_json::json!({ "text": "hi" }));
    }

    #[test]
    fn data_only_frame_has_no_event_name() {
        let frames = frames_of("data: {\"choices\":[]}\n\n");
        assert_eq!(frames.len(), 1);
        assert!(frames[0].event.is_none());
        assert_eq!(frames[0].data, serde_json::json!({ "choices": [] }));
    }

    #[test]
    fn done_sentinel_degrades_to_string_payload() {
        let frames = frames_of("data: [DONE]\n\n");
        assert_eq!(frames[0].data, Value::String("[DONE]".to_owned()));
    }

    #[test]
    fn multi_data_lines_join_with_newline() {
        let frames = frames_of("data: {\"a\":\ndata: 1}\n\n");
        assert_eq!(frames[0].data, serde_json::json!({ "a": 1 }));
    }

    #[test]
    fn crlf_endings_parse_like_lf() {
        let frames = frames_of("event: ping\r\ndata: {}\r\n\r\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event.as_deref(), Some("ping"));
    }

    #[test]
    fn comment_and_event_only_blocks_yield_no_frame() {
        let frames = frames_of(": keep-alive\n\nevent: name_only\ndata: 7\n\n");
        assert_eq!(frames.len(), 1, "only the data-carrying block is a frame");
        assert_eq!(frames[0].data, serde_json::json!(7));
    }

    #[test]
    fn byte_by_byte_chunks_preserve_frames() {
        let text = "event: a\ndata: {\"x\":1}\n\ndata: [DONE]\n\n";
        let mut parser = SseFrameParser::new();
        for byte in text.bytes() {
            parser.feed(&[byte]).expect("no overflow on small input");
        }
        let first = parser.next_frame().expect("first frame");
        assert_eq!(first.event.as_deref(), Some("a"));
        assert!(parser.next_frame().is_some(), "[DONE] frame");
        assert!(parser.next_frame().is_none());
    }

    #[test]
    fn frame_split_across_three_feeds_emits_exactly_once() {
        let mut parser = SseFrameParser::new();
        parser.feed(b"event: delta\nda").expect("feed 1");
        assert!(parser.next_frame().is_none(), "partial frame must not emit");
        parser.feed(b"ta: {\"x\":").expect("feed 2");
        assert!(parser.next_frame().is_none(), "still partial");
        parser.feed(b"1}\n\n").expect("feed 3");
        let frame = parser.next_frame().expect("complete after 3rd feed");
        assert_eq!(frame.event.as_deref(), Some("delta"));
        assert_eq!(frame.data, serde_json::json!({ "x": 1 }));
        assert!(parser.next_frame().is_none(), "emitted exactly once");
    }

    #[test]
    fn many_frames_in_one_feed_match_frame_by_frame_feeds() {
        // Mirrors provider framing: many events per hyper chunk. Even
        // indices use CRLF so normalization is pinned in the batch path.
        let frame_text = |i: usize| {
            let line_ending = if i.is_multiple_of(2) { "\r\n" } else { "\n" };
            format!("event: delta\ndata: {{\"i\":{i}}}{line_ending}{line_ending}")
        };
        let batch_text: String = (0..200).map(frame_text).collect();
        let batch = frames_of(&batch_text);
        assert_eq!(batch.len(), 200, "every batched frame extracted");
        for (i, frame) in batch.iter().enumerate() {
            assert_eq!(frame.event.as_deref(), Some("delta"), "event at {i}");
            assert_eq!(frame.data, serde_json::json!({ "i": i }), "data at {i}");
        }

        let mut parser = SseFrameParser::new();
        for i in 0..200 {
            parser.feed(frame_text(i).as_bytes()).expect("parse");
        }
        let mut one_by_one = Vec::new();
        while let Some(frame) = parser.next_frame() {
            one_by_one.push(frame);
        }
        assert_eq!(one_by_one.len(), 200);
        for (i, (b, o)) in batch.iter().zip(&one_by_one).enumerate() {
            assert_eq!(
                b.event.as_deref(),
                o.event.as_deref(),
                "event mode mismatch at {i}"
            );
            assert_eq!(b.data, o.data, "data mode mismatch at {i}");
        }
    }

    #[test]
    fn unbounded_stream_without_boundary_is_a_protocol_violation() {
        let mut parser = SseFrameParser::new();
        let verdict = parser.feed(&vec![b'x'; MAX_FRAME_BYTES + 1]);
        assert!(verdict.is_err(), "buffer must be bounded");
        assert!(matches!(verdict, Err(TransportFailure::Protocol(_))));
    }

    #[test]
    fn optional_single_space_after_colon_is_stripped_once() {
        let frames = frames_of("data:  42\n\n");
        // One space is the separator; the second is payload — but JSON
        // tolerates leading whitespace, so this still parses as a number.
        assert_eq!(frames[0].data, serde_json::json!(42));
    }
}
