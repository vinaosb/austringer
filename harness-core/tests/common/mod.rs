//! Shared helpers for harness-core integration tests.
//!
//! The event generator is deliberately dependency-free: a hand-rolled
//! xorshift64* PRNG with fixed seeds keeps property tests deterministic (no
//! flaky runs, no wall-clock or rand-version dependence) while still covering
//! the full event taxonomy with structurally diverse payloads.
#![allow(dead_code)]

pub mod cancelkit;
pub mod loopkit;
pub mod stages_kit;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use harness_core::event::content::{ContentBlock, ImageBlock, MimeType};
use harness_core::event::{ContentHash, Event, MediaRef, Sequence};

/// Deterministic xorshift64* PRNG (Marsaglia). Never used outside tests.
pub struct XorShift64(u64);

impl XorShift64 {
    /// New generator. The seed is forced odd so the state never hits zero.
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// Next raw 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish value in `0..n` (modulo bias is irrelevant for test data).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// Random string built from a nasty alphabet: CJK, emoji, quotes,
    /// backslashes, and control characters to stress serde escaping.
    pub fn string(&mut self, max_len: u64) -> String {
        const ALPHABET: &[&str] = &[
            "a",
            "Z",
            "0",
            " ",
            "\n",
            "\t",
            "\u{1}",
            "\"",
            "\\",
            "/",
            "{",
            "}",
            "[",
            "]",
            ":",
            ",",
            "漢",
            "字",
            "🦀",
            "é",
            "ß",
            "😀",
            "рж",
            "uncommitted",
            "line\nbreak",
        ];
        let len = self.below(max_len + 1) as usize;
        (0..len)
            .map(|_| ALPHABET[self.below(ALPHABET.len() as u64) as usize])
            .collect()
    }

    /// Random JSON value (objects exercise sorted-key serialization).
    pub fn json_value(&mut self, depth: u64) -> serde_json::Value {
        use serde_json::Value;
        match self.below(if depth == 0 { 4 } else { 6 }) {
            0 => Value::Null,
            1 => Value::Bool(self.next_u64() & 1 == 0),
            2 => Value::from(self.next_u64() as i64),
            3 => Value::String(self.string(24)),
            4 => Value::Array(
                (0..self.below(4))
                    .map(|_| self.json_value(depth - 1))
                    .collect(),
            ),
            // Insert keys in scrambled order; stable serialization must emit
            // them sorted (serde_json's default BTreeMap ordering).
            _ => {
                let keys = [
                    self.string(8),
                    self.string(8),
                    format!("zz_{}", self.below(100)),
                    format!("aa_{}", self.below(100)),
                ];
                let mut object = serde_json::Map::new();
                for key in keys {
                    object.insert(key, self.json_value(depth - 1));
                }
                Value::Object(object)
            }
        }
    }

    /// Random image block with an arbitrary (not necessarily stored) hash.
    pub fn image_block(&mut self) -> ImageBlock {
        let mime = match self.below(4) {
            0 => MimeType::png(),
            1 => MimeType::jpeg(),
            2 => MimeType::gif(),
            _ => MimeType::webp(),
        };
        let mut bytes = vec![0u8; 64];
        for byte in &mut bytes {
            *byte = self.next_u64() as u8;
        }
        ImageBlock {
            mime,
            width: self.below(4000) as u32,
            height: self.below(4000) as u32,
            media: MediaRef {
                hash: ContentHash::from_bytes(&bytes),
                bytes: self.below(1 << 20),
            },
        }
    }

    /// Random content-block vector mixing text and image blocks.
    pub fn blocks(&mut self) -> Vec<ContentBlock> {
        (0..self.below(4))
            .map(|_| {
                if self.below(3) == 0 {
                    ContentBlock::Image(self.image_block())
                } else {
                    ContentBlock::Text {
                        text: self.string(40),
                    }
                }
            })
            .collect()
    }

    /// Random event covering the full seven-kind taxonomy.
    pub fn event(&mut self) -> Event {
        match self.below(7) {
            0 => Event::User {
                blocks: self.blocks(),
            },
            1 => Event::Assistant {
                blocks: self.blocks(),
            },
            2 => Event::ToolCall {
                call_id: self.string(12),
                tool: self.string(10),
                arguments: self.json_value(3),
            },
            3 => Event::ToolResult {
                call_id: self.string(12),
                blocks: self.blocks(),
            },
            4 => Event::System {
                text: self.string(60),
            },
            5 => Event::Condensation {
                summary: self.string(80),
                through_seq: Sequence(self.below(1000)),
            },
            _ => Event::Snapshot {
                label: self.string(20),
            },
        }
    }
}

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique session directory under the system temp dir, removed on drop.
pub struct TempSession(PathBuf);

impl TempSession {
    /// Create the session directory (uniqueness: test name + pid + counter).
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "harness-core-t4-{name}-{}-{}",
            std::process::id(),
            DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("temp session dir creation");
        Self(path)
    }

    /// The session directory path.
    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempSession {
    fn drop(&mut self) {
        // Best-effort cleanup; a failed remove never fails the test.
        let _ = fs::remove_dir_all(&self.0);
    }
}
