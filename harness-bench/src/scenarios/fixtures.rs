//! Deterministic scenario fixtures: one seeded PRNG, event builders, and
//! the mock eviction seams. Same seed -> byte-identical inputs, so every
//! deterministic metric is exactly reproducible.

use async_trait::async_trait;
use harness_core::context::{ContextSpec, EffortLevel, EffortSettings, SlotSegment};
use harness_core::event::{ContentBlock, Event, RecordedEvent, Sequence};
use harness_core::eviction::{
    CondenseError, CondenseRequest, Condenser, ReturnCode, SalienceFilter, ToolReturnTable,
};

/// xorshift64* — same family as the engine test kits; seed once, reuse.
pub struct Rng(u64);

impl Rng {
    pub const fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// Pseudo-text of `chunks` 7-char words (printable ASCII).
    pub fn text(&mut self, chunks: u64) -> String {
        (0..chunks)
            .map(|_| {
                let w = self.next_u64();
                (0..7)
                    .map(|i| (b'a' + ((w >> (i * 9)) % 26) as u8) as char)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub fn user_event(seq: u64, text: &str) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event: Event::User {
            blocks: vec![ContentBlock::Text {
                text: text.to_owned(),
            }],
        },
    }
}

pub fn assistant_event(seq: u64, text: &str) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event: Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: text.to_owned(),
            }],
        },
    }
}

pub fn tool_pair(seq: u64, tool: &str, result: &str) -> Vec<RecordedEvent> {
    vec![
        RecordedEvent {
            seq: Sequence(seq),
            event: Event::ToolCall {
                call_id: format!("call-{seq}"),
                tool: tool.to_owned(),
                arguments: serde_json::json!({"path": format!("src/mod{seq}.rs")}),
            },
        },
        RecordedEvent {
            seq: Sequence(seq + 1),
            event: Event::ToolResult {
                call_id: format!("call-{seq}"),
                blocks: vec![ContentBlock::Text {
                    text: result.to_owned(),
                }],
            },
        },
    ]
}

/// Mixed user/assistant/tool history large enough to trip a small window.
pub fn synthetic_history(rng: &mut Rng, turns: u64) -> Vec<RecordedEvent> {
    let mut events = Vec::new();
    let mut seq = 1u64;
    for turn in 0..turns {
        events.push(user_event(seq, &format!("turn {turn}: {}", rng.text(12))));
        seq += 1;
        events.push(assistant_event(seq, &rng.text(60)));
        seq += 1;
        events.extend(tool_pair(seq, "view_window", &rng.text(90)));
        seq += 2;
    }
    events
}

/// Tool-dominant history (the eviction walk can only fold tool results;
/// free text mass belongs to the todo-13 compactor). Mirrors the engine
/// kit's `bloated_tool_history` shape: 3 protected-head prose events,
/// then pure tool pairs — no interleaved prose in the foldable middle.
pub fn bloated_tool_history(rng: &mut Rng, pairs: u64) -> Vec<RecordedEvent> {
    let mut events = Vec::new();
    let mut seq = 1u64;
    for index in 0..3u64 {
        events.push(user_event(seq, &format!("head user message {index}")));
        seq += 1;
    }
    for _ in 0..pairs {
        let pair = tool_pair(seq, "execute_command", &rng.text(120));
        seq += pair.len() as u64;
        events.extend(pair);
    }
    events
}

/// Return codes for every tool call in `history` (folding refuses to
/// drop codes it cannot resolve — todo-12 doctrine).
pub fn tool_returns(history: &[RecordedEvent]) -> ToolReturnTable {
    let mut table = ToolReturnTable::default();
    for event in history {
        if let Event::ToolCall { call_id, .. } = &event.event {
            table.0.insert(call_id.clone(), ReturnCode(0));
        }
    }
    table
}

/// A prompt spec with populated frozen slots + history.
pub fn context_spec(history: Vec<RecordedEvent>) -> ContextSpec {
    ContextSpec {
        system: "You are a general-purpose coding agent (harness).".to_owned(),
        base_schema: "{\"core_tools\":[\"view_window\",\"edit_lines\",\"execute_command\",\"lsp_query\",\"tool_search\",\"task_completed\"]}".to_owned(),
        rules: vec![
            SlotSegment { id: "rule-00".to_owned(), content: "Prefer windows over whole files.".to_owned() },
            SlotSegment { id: "rule-01".to_owned(), content: "Never rewrite whole files.".to_owned() },
        ],
        skills: vec![SlotSegment {
            id: "skill-rust".to_owned(),
            content: "Rust discipline: strict types, no unwrap in src.".to_owned(),
        }],
        repomap: "src/core.rs:\n    Engine:struct\n    step:fn\nsrc/exec.rs:\n    PtyRunner:struct"
            .to_owned(),
        history,
        last_user_message: "Continue: run the failing test and fix the root cause.".to_owned(),
        effort: EffortSettings { level: EffortLevel::Medium, thinking_budget: None },
        beta_effort: None,
    }
}

/// Deterministic condenser (zero LLM): summary of fixed shape.
pub struct FixedCondenser;

#[async_trait]
impl Condenser for FixedCondenser {
    async fn condense(&self, request: CondenseRequest<'_>) -> Result<String, CondenseError> {
        Ok(format!(
            "<condensate>{} events, {} bytes; facts retained by test fixture</condensate>",
            request.span.len(),
            request.span_bytes
        ))
    }
}

/// No salient facts (keeps the fixture output independent of heuristics).
pub struct NoSalience;

impl SalienceFilter for NoSalience {
    fn salient(&self, _event: &RecordedEvent) -> Vec<String> {
        Vec::new()
    }
}
