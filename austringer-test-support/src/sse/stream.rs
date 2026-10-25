//! Chunked SSE byte stream with mid-stream drop, clean close-after-N, and
//! inter-chunk delay injection (idle-timeout paths). The stream is pure
//! in-memory state plus tokio timers — no sockets.

use std::collections::VecDeque;
use std::time::Duration;

use serde_json::Value;

use super::SseEvent;
use super::parser::SseParser;
use super::script::SseScript;

/// One transport chunk: the rendered bytes of a single event plus the delay to
/// sleep BEFORE yielding it (gaps accumulate from preceding delay steps).
#[derive(Debug)]
struct Chunk {
    bytes: Vec<u8>,
    delay: Duration,
    /// Ordinal of the event this chunk carries (used by `close_after_events`).
    ordinal: usize,
}

/// In-memory byte stream mimicking a chunked HTTP response body, with
/// controllable failure points.
#[derive(Debug)]
pub struct SseByteStream {
    chunks: VecDeque<Chunk>,
}

impl SseByteStream {
    /// Builds the full scripted stream; yields one chunk per event.
    pub fn from_script(script: &SseScript) -> Self {
        let mut chunks = VecDeque::new();
        let mut pending_delay = Duration::ZERO;
        let mut ordinal = 0;
        for step in &script.steps {
            match step {
                super::script::SseStep::Event(ev) => {
                    chunks.push_back(Chunk {
                        bytes: ev.render().into_bytes(),
                        delay: pending_delay,
                        ordinal,
                    });
                    ordinal += 1;
                    pending_delay = Duration::ZERO;
                }
                super::script::SseStep::Delay(gap) => {
                    pending_delay += *gap;
                }
            }
        }
        Self { chunks }
    }

    /// Truncates the stream mid-event after exactly `n` bytes total — the final
    /// chunk is torn so consumers observe a PARTIAL event stream, exactly like a
    /// dropped connection. `n` beyond the stream length keeps everything.
    pub fn drop_after_bytes(mut self, n: u64) -> Self {
        let mut kept: VecDeque<Chunk> = VecDeque::new();
        let mut acc: u64 = 0;
        for chunk in self.chunks.drain(..) {
            let len = chunk.bytes.len() as u64;
            if acc + len <= n {
                acc += len;
                kept.push_back(chunk);
                continue;
            }
            let remain = (n - acc) as usize;
            if remain > 0 {
                let Chunk {
                    bytes,
                    delay,
                    ordinal,
                } = chunk;
                kept.push_back(Chunk {
                    bytes: bytes[..remain].to_vec(),
                    delay,
                    ordinal,
                });
            }
            break;
        }
        self.chunks = kept;
        self
    }

    /// Ends the stream cleanly after `n` complete events (no torn frame);
    /// anything later in the script — including a trailing `done` — never
    /// arrives.
    pub fn close_after_events(mut self, n: usize) -> Self {
        self.chunks.retain(|c| c.ordinal < n);
        self
    }

    /// Yields the next chunk, sleeping its pre-delay first. Under
    /// `start_paused` the sleep is virtual, so tests never really wait.
    pub async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        let chunk = self.chunks.pop_front()?;
        if !chunk.delay.is_zero() {
            tokio::time::sleep(chunk.delay).await;
        }
        Some(chunk.bytes)
    }

    /// Collects every remaining chunk into one byte buffer.
    pub async fn read_to_end(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(chunk) = self.next_chunk().await {
            out.extend_from_slice(&chunk);
        }
        out
    }

    /// Consumes the stream and returns `(joined text deltas, complete events)`.
    /// A mid-event drop is visible as fewer events, not an error.
    pub async fn read_all(self) -> (String, Vec<SseEvent>) {
        let mut stream = self;
        let mut parser = SseParser::new();
        while let Some(chunk) = stream.next_chunk().await {
            parser.feed(&chunk);
        }
        let text = parser
            .events()
            .iter()
            .filter(|e| e.event == "text_delta")
            .filter_map(|e| e.data.get("text").and_then(Value::as_str))
            .collect();
        (text, parser.events().to_vec())
    }
}
