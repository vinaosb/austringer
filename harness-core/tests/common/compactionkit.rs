//! Shared fixtures for the compaction integration tests (plan todo 13).
//!
//! Deterministic by construction: the doctrine session is fully scripted
//! (no PRNG, no clock, no filesystem outside the caller's control), the
//! mock seams are pure, and the seeded generator rides the shared
//! [`super::XorShift64`] PRNG for the adversarial passes.

#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use harness_core::agent_loop::CancelToken;
use harness_core::compaction::condense::{
    CondenseRefusal, DoctrineCondenseRequest, DoctrineCondenser, ModelId, SessionCondenser,
};
use harness_core::compaction::engine::{CompactionSink, HookError};
use harness_core::compaction::handoff::{HandoffBundle, SalientPointer, ThreadExtractor};
use harness_core::event::content::ContentBlock;
use harness_core::event::{Event, RecordedEvent, Sequence};
use harness_core::eviction::{CondenseError, ReturnCode, ToolReturnTable};

fn text(content: &str) -> ContentBlock {
    ContentBlock::Text {
        text: content.to_owned(),
    }
}

fn recorded(seq: u64, event: Event) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event,
    }
}

fn user(seq: u64, content: &str) -> RecordedEvent {
    recorded(
        seq,
        Event::User {
            blocks: vec![text(content)],
        },
    )
}

fn assistant(seq: u64, content: &str) -> RecordedEvent {
    recorded(
        seq,
        Event::Assistant {
            blocks: vec![text(content)],
        },
    )
}

fn system(seq: u64, content: &str) -> RecordedEvent {
    recorded(
        seq,
        Event::System {
            text: content.to_owned(),
        },
    )
}

fn tool_pair(seq: u64, call_id: &str, tool: &str, path: &str, payload: &str) -> Vec<RecordedEvent> {
    vec![
        recorded(
            seq,
            Event::ToolCall {
                call_id: call_id.to_owned(),
                tool: tool.to_owned(),
                arguments: serde_json::json!({ "path": path }),
            },
        ),
        recorded(
            seq + 1,
            Event::ToolResult {
                call_id: call_id.to_owned(),
                blocks: vec![text(payload)],
            },
        ),
    ]
}

/// The doctrine session: 36 events where the known facts (API port,
/// database, hash decision, unresolved bug) live EARLY — outside the
/// last-10-turns window — so they survive compaction only through the
/// doctrine path, never through the protected tail.
pub fn facts_session() -> Vec<RecordedEvent> {
    let mut events = Vec::new();
    events.push(user(
        1,
        "We are building the billing service.\nDECISION: the API port is 8085.\n\
         DECISION: the database is Postgres 15.",
    ));
    events.push(assistant(2, "Noted the two decisions."));
    events.push(user(3, "DECISION: use blake3 for all content hashes."));
    events.push(system(
        4,
        &format!(
            "<subagent_summary name=\"scout\">{}\n</subagent_summary>",
            "scout found the ledger module. ".repeat(600)
        ),
    ));
    events.extend(tool_pair(
        5,
        "call-01",
        "view_window",
        "src/billing/api.rs",
        "impl Api {}",
    ));
    events.push(system(
        7,
        "UNRESOLVED BUG: checkout returns 500 under concurrent load.",
    ));
    events.extend(tool_pair(
        8,
        "call-02",
        "view_window",
        "src/billing/ledger.rs",
        "mod ledger {}",
    ));
    events.extend(tool_pair(
        10,
        "call-03",
        "view_window",
        "src/billing/cache.rs",
        "mod cache {}",
    ));
    // Filler turns (each turn = user + assistant pair), seq 12..31.
    for turn in 4..=13u64 {
        let base = 12 + (turn - 4) * 2;
        if turn == 8 {
            events.push(system(base, "<milestone label=\"billing-mvp\"/>"));
            events.push(assistant(base + 1, "milestone recorded"));
            continue;
        }
        events.push(user(
            base,
            &format!("filler question {turn} about invoices"),
        ));
        events.push(assistant(
            base + 1,
            &format!("filler answer {turn}: {}", "analysis ".repeat(40)),
        ));
    }
    // Recent tail (last ~2 turns) includes a fresh file + a fresh fact.
    events.extend(tool_pair(
        32,
        "call-04",
        "edit_lines",
        "src/checkout/mod.rs",
        "edited checkout",
    ));
    events.push(user(34, "the cache TTL is 90 seconds"));
    events.push(assistant(35, "cache TTL noted"));
    events.push(user(36, "please continue"));
    events
}

/// Authoritative return-code table for a fixture history (all zero).
pub fn session_tool_returns(events: &[RecordedEvent]) -> ToolReturnTable {
    let mut table = ToolReturnTable::default();
    for (index, entry) in events.iter().enumerate() {
        if let Event::ToolResult { call_id, .. } = &entry.event {
            table
                .0
                .insert(call_id.clone(), ReturnCode((index % 3) as i32));
        }
    }
    table
}

/// The session's model identity used across the compaction tests.
pub fn session_model() -> ModelId {
    ModelId::new("anthropic/claude-x")
}

/// Rule-based doctrine condenser (the scripted model): its summary
/// CARRIES every DECISION/FACT/UNRESOLVED line of the span verbatim —
/// the behavior the replay-equivalence acceptance pins.
pub struct ScriptedCondenser;

#[async_trait::async_trait]
impl DoctrineCondenser for ScriptedCondenser {
    async fn condense(
        &self,
        request: DoctrineCondenseRequest<'_>,
    ) -> Result<String, CondenseError> {
        let mut carried = Vec::new();
        for entry in request.span {
            let blocks = match &entry.event {
                Event::User { blocks } | Event::Assistant { blocks } => blocks.as_slice(),
                Event::System { text } => {
                    for line in text.lines() {
                        if line.starts_with("DECISION:") || line.starts_with("UNRESOLVED BUG:") {
                            carried.push(line.to_owned());
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            for block in blocks {
                if let ContentBlock::Text { text: body } = block {
                    for line in body.lines() {
                        if line.starts_with("DECISION:")
                            || line.starts_with("UNRESOLVED BUG:")
                            || line.starts_with("FACT:")
                        {
                            carried.push(line.to_owned());
                        }
                    }
                }
            }
        }
        Ok(format!(
            "condensed {} events; carried:\n{}",
            request.span.len(),
            carried.join("\n")
        ))
    }
}

/// Always-failing condenser: forces the deterministic fallback.
pub struct FailingCondenser;

#[async_trait::async_trait]
impl DoctrineCondenser for FailingCondenser {
    async fn condense(
        &self,
        _request: DoctrineCondenseRequest<'_>,
    ) -> Result<String, CondenseError> {
        Err(CondenseError(
            "provider 503: condense unavailable".to_owned(),
        ))
    }
}

/// Condenser that counts calls — asserts the overflow path NEVER touches
/// an LLM seam.
pub struct CountingCondenser {
    pub calls: AtomicUsize,
}

impl CountingCondenser {
    #[must_use]
    pub fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }

    #[must_use]
    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Default for CountingCondenser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl DoctrineCondenser for CountingCondenser {
    async fn condense(
        &self,
        _request: DoctrineCondenseRequest<'_>,
    ) -> Result<String, CondenseError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("counted condense".to_owned())
    }
}

/// Wraps a condenser for the given session model.
pub fn session_condenser(model: &ModelId, inner: Arc<dyn DoctrineCondenser>) -> SessionCondenser {
    SessionCondenser::for_session(model.clone(), inner)
}

/// Refusal must be a model mismatch with both identities.
#[must_use]
pub fn is_model_mismatch(refusal: &CondenseRefusal) -> bool {
    matches!(refusal, CondenseRefusal::ModelMismatch(_))
}

/// Mock second-model extractor for read_thread-style handoffs.
pub struct MockExtractor;

#[async_trait::async_trait]
impl ThreadExtractor for MockExtractor {
    async fn extract_thread(
        &self,
        events: &[RecordedEvent],
    ) -> Result<HandoffBundle, CondenseError> {
        Ok(HandoffBundle {
            distilled_summary: format!("extracted {} events", events.len()),
            salient_pointers: vec![SalientPointer {
                seq: events.last().map_or(Sequence(0), |entry| entry.seq),
                note: "thread tail".to_owned(),
            }],
        })
    }
}

/// Failing extractor: the seam error path.
pub struct FailingExtractor;

#[async_trait::async_trait]
impl ThreadExtractor for FailingExtractor {
    async fn extract_thread(
        &self,
        _events: &[RecordedEvent],
    ) -> Result<HandoffBundle, CondenseError> {
        Err(CondenseError("extractor offline".to_owned()))
    }
}

/// In-memory T4 sink recording appended events with fresh sequences.
pub struct RecordingSink {
    /// Appended events in order.
    pub appended: Vec<Event>,
    next: u64,
}

impl RecordingSink {
    /// A sink whose numbering continues after `events`.
    #[must_use]
    pub fn after(events: &[RecordedEvent]) -> Self {
        Self {
            appended: Vec::new(),
            next: events.last().map_or(0, |entry| entry.seq.0),
        }
    }

    /// The appended events paired with their assigned sequences.
    #[must_use]
    pub fn recorded(&self) -> Vec<RecordedEvent> {
        self.appended
            .iter()
            .enumerate()
            .map(|(index, event)| RecordedEvent {
                seq: Sequence(self.next - self.appended.len() as u64 + index as u64 + 1),
                event: event.clone(),
            })
            .collect()
    }
}

impl CompactionSink for RecordingSink {
    fn append(&mut self, event: Event) -> Result<Sequence, HookError> {
        self.appended.push(event.clone());
        self.next += 1;
        Ok(Sequence(self.next))
    }
}

/// The mock inference seam for replay-equivalence: it sees exactly the
/// compacted prompt the next turn would send.
pub struct MockInference {
    /// The compacted context the next inference consumes.
    pub prompt: String,
}

impl MockInference {
    /// Wraps the compacted prompt.
    #[must_use]
    pub fn new(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_owned(),
        }
    }

    /// Whether the compacted prompt carries a pre-compaction fact.
    #[must_use]
    pub fn carries(&self, fact: &str) -> bool {
        self.prompt.contains(fact)
    }
}

/// Seeded adversarial session: unicode-heavy payloads, one huge single
/// turn, marker facts sprinkled deterministically.
pub fn seeded_session(seed: u64, total: usize) -> Vec<RecordedEvent> {
    let mut rng = super::XorShift64::new(seed);
    let mut events = Vec::new();
    let mut seq = 0u64;
    for index in 0..total {
        seq += 1;
        let event = match index % 7 {
            0 => Event::User {
                blocks: vec![ContentBlock::Text {
                    text: format!("turn {index} 初音ミク🌱\nDECISION: seed fact {seed}/{index}"),
                }],
            },
            1 => Event::Assistant {
                blocks: vec![ContentBlock::Text {
                    text: rng.string(60),
                }],
            },
            2 => Event::System {
                text: format!("UNRESOLVED BUG: 🐛 {} in module {index}", rng.string(12)),
            },
            3 if index > 3 && index + 4 < total => Event::ToolCall {
                call_id: format!("seed-call-{index}"),
                tool: "view_window".to_owned(),
                arguments: serde_json::json!({ "path": format!("src/mod{index}.rs") }),
            },
            4 if index > 3 && index + 4 < total => Event::ToolResult {
                call_id: format!("seed-call-{index}"),
                blocks: vec![ContentBlock::Text {
                    text: " FACT: output 初音ミク\n".repeat(30),
                }],
            },
            5 if index == 5 => Event::User {
                // The huge single turn: one event larger than the rest combined.
                blocks: vec![ContentBlock::Text {
                    text: format!(
                        "DECISION: huge turn survives trimmed\n{}",
                        "🦀 é ß рж".repeat(20_000)
                    ),
                }],
            },
            _ => Event::Assistant {
                blocks: vec![ContentBlock::Text {
                    text: format!("reply {index}"),
                }],
            },
        };
        events.push(recorded(seq, event));
    }
    events
}

/// An uncancelled token bundle for engine deps.
pub struct CancelSeam {
    /// The shared token.
    pub token: CancelToken,
}

impl CancelSeam {
    /// Fresh, uncancelled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            token: CancelToken::new(),
        }
    }
}

impl Default for CancelSeam {
    fn default() -> Self {
        Self::new()
    }
}
