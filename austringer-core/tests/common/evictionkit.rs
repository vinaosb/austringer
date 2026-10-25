//! Shared fixtures for the eviction integration tests (plan todo 12).
//!
//! Deterministic by construction: one seeded PRNG drives every fixture,
//! every mock seam is pure, and no fixture touches the filesystem or the
//! clock.

#![allow(dead_code)]

pub use super::XorShift64 as EviRng;

use austringer_core::event::content::ContentBlock;
use austringer_core::event::{Event, RecordedEvent, Sequence};
use austringer_core::eviction::{
    ActivePlan, CondenseRequest, Condenser, EvictionConfig, EvictionDeps, EvictionInput,
    MemoryHook, ReturnCode, SalienceFilter, ToolReturnTable,
};

/// Default test window (tokens) for tier/structure tests.
pub const TEST_WINDOW: u64 = 8_000;

/// A config with the given window and default policy everywhere else.
pub fn config(window: u64) -> EvictionConfig {
    EvictionConfig::default().with_window(window)
}

fn text(content: String) -> ContentBlock {
    ContentBlock::Text { text: content }
}

fn recorded(seq: u64, event: Event) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event,
    }
}

/// Mixed synthetic history: user/assistant/system conversation plus tool
/// call/result pairs interleaved (small payloads — for structure tests).
pub fn synthetic_history(rng: &mut EviRng, total: usize) -> Vec<RecordedEvent> {
    let mut history = Vec::with_capacity(total);
    let mut call_no = 0u64;
    for index in 0..total {
        let seq = (index + 1) as u64;
        let event = match index % 5 {
            0 => Event::User {
                blocks: vec![text(format!("user message {index}"))],
            },
            1 => Event::Assistant {
                blocks: vec![text(format!("assistant reply {index}"))],
            },
            2 if index >= 3 && index + 20 < total => {
                call_no += 1;
                Event::ToolCall {
                    call_id: format!("call-{call_no:04}"),
                    tool: "view_window".to_owned(),
                    arguments: serde_json::json!({"path": format!("src/file{index}.rs")}),
                }
            }
            3 if index >= 3 && index + 20 < total => {
                call_no += 1;
                let call_id = format!("call-{call_no:04}");
                Event::ToolResult {
                    call_id,
                    blocks: vec![text(format!("line {} of output", rng.below(100)))],
                }
            }
            4 => Event::System {
                text: format!("system notice {index}"),
            },
            _ => Event::User {
                blocks: vec![text(format!("spare user message {index}"))],
            },
        };
        history.push(recorded(seq, event));
    }
    history
}

/// History dominated by large middle-zone tool results (for ladder and
/// folding tests): 3 small head events, big tool pairs in the middle,
/// 20 small tail events.
pub fn bloated_tool_history(rng: &mut EviRng, total: usize) -> Vec<RecordedEvent> {
    let mut history = Vec::with_capacity(total + 4);
    let mut seq = 0u64;
    let mut call_no = 0u64;
    for index in 0..3 {
        seq += 1;
        history.push(recorded(
            seq,
            Event::User {
                blocks: vec![text(format!("head user message {index}"))],
            },
        ));
    }
    let mut index = 3;
    while index + 20 < total {
        call_no += 1;
        let call_id = format!("call-{call_no:04}");
        seq += 1;
        history.push(recorded(
            seq,
            Event::ToolCall {
                call_id: call_id.clone(),
                tool: "execute_command".to_owned(),
                arguments: serde_json::json!({"cmd": "cargo test"}),
            },
        ));
        let size = 2_000 + rng.below(2_000) as usize;
        seq += 1;
        history.push(recorded(
            seq,
            Event::ToolResult {
                call_id,
                blocks: vec![text(format!(
                    "FACT: run {call_no} finished\n{}",
                    "x".repeat(size)
                ))],
            },
        ));
        index += 2;
    }
    while history.len() < total {
        seq += 1;
        history.push(recorded(
            seq,
            Event::Assistant {
                blocks: vec![text(format!("tail note {}", history.len()))],
            },
        ));
    }
    history
}

/// History dominated by large middle-zone tool results whose payloads are
/// unicode-heavy (CJK + emoji + control characters) to stress folding.
pub fn unicode_tool_history(rng: &mut EviRng, total: usize) -> Vec<RecordedEvent> {
    let mut history = bloated_tool_history(rng, total);
    let mut counter = 0u32;
    for recorded in &mut history {
        if let Event::ToolResult { blocks, .. } = &mut recorded.event {
            counter += 1;
            let payload: String = "初音ミク🌱\u{1}".repeat(120 + counter as usize);
            blocks[0] = ContentBlock::Text {
                text: format!("FACT: unicode run {counter}\n{payload}"),
            };
        }
    }
    history
}

/// A tiny well-below-budget history.
pub fn small_history(rng: &mut EviRng) -> Vec<RecordedEvent> {
    synthetic_history(rng, 6)
}

/// The authoritative return-code table for a fixture history: code =
/// position of the result modulo 7 (deterministic, sometimes nonzero).
pub fn tool_returns(history: &[RecordedEvent]) -> ToolReturnTable {
    let mut table = ToolReturnTable::default();
    for (index, recorded) in history.iter().enumerate() {
        if let Event::ToolResult { call_id, .. } = &recorded.event {
            table
                .0
                .insert(call_id.clone(), ReturnCode((index % 7) as i32));
        }
    }
    table
}

/// A plan block for inputs (Tier0).
pub fn plan() -> ActivePlan {
    ActivePlan {
        text: "1. write tests\n2. implement\n3. verify".to_owned(),
    }
}

/// An input with the default policy ladder and the given window.
pub fn input_with_window(
    history: &[RecordedEvent],
    window: u64,
) -> (EvictionConfig, EvictionInput) {
    let config = config(window);
    let input = EvictionInput::new(history.to_vec())
        .with_plan(plan())
        .with_tool_returns(tool_returns(history));
    (config, input)
}

/// An input whose window is tuned so usage lands strictly between the
/// flush threshold and the eviction trigger (the pre-compact flush band).
pub fn flush_window_input(history: &[RecordedEvent]) -> (EvictionConfig, EvictionInput) {
    let probe = EvictionInput::new(history.to_vec()).with_plan(plan());
    let usage = probe.used_tokens();
    // 72% of the window: above flush-at (75% - lead) and below 75% for
    // any sane lead (>= 3% of the window).
    let window = (usage * 100 / 72).max(1);
    let config = config(window);
    let input = probe.with_tool_returns(tool_returns(history));
    (config, input)
}

/// The default engine under test.
pub fn default_engine() -> austringer_core::eviction::DefaultEvictionEngine {
    austringer_core::eviction::DefaultEvictionEngine::new()
}

/// The second, minimal engine proving the trait is pluggable.
pub fn minimal_engine() -> austringer_core::eviction::MinimalEngine {
    austringer_core::eviction::MinimalEngine
}

/// Deterministic mock condenser: summarizes a span by count + content
/// hash (no model, no clock).
pub struct FixedCondenser;

#[async_trait::async_trait]
impl Condenser for FixedCondenser {
    async fn condense(
        &self,
        request: CondenseRequest<'_>,
    ) -> Result<String, austringer_core::eviction::CondenseError> {
        Ok(format!(
            "condensed {} events ({} bytes)",
            request.span.len(),
            request.span_bytes
        ))
    }
}

/// Deterministic mock salience filter: FACT-prefixed lines are salient.
pub struct PrefixSalience;

impl SalienceFilter for PrefixSalience {
    fn salient(&self, event: &RecordedEvent) -> Vec<String> {
        let mut facts = Vec::new();
        let blocks = match &event.event {
            Event::ToolResult { blocks, .. } => blocks,
            Event::User { blocks } | Event::Assistant { blocks } => blocks,
            _ => return facts,
        };
        for block in blocks {
            if let ContentBlock::Text { text: content } = block {
                for line in content.lines() {
                    if line.starts_with("FACT:") {
                        facts.push(line.to_owned());
                    }
                }
            }
        }
        facts
    }
}

/// In-memory memory hook collecting flushes (assertion surface).
pub struct VecMemoryHook {
    /// Every flush received, in order.
    pub flushed: Vec<Vec<String>>,
}

impl MemoryHook for VecMemoryHook {
    fn flush(&mut self, facts: Vec<String>) {
        self.flushed.push(facts);
    }
}

/// Owned bundle of the three mock seams with an assertion surface.
pub struct TestDeps {
    /// The collected memory flushes.
    pub memory: VecMemoryHook,
    condenser: FixedCondenser,
    salience: PrefixSalience,
}

impl TestDeps {
    /// Fresh mocks.
    pub fn new() -> Self {
        Self {
            memory: VecMemoryHook {
                flushed: Vec::new(),
            },
            condenser: FixedCondenser,
            salience: PrefixSalience,
        }
    }

    /// Borrows the seams as engine deps.
    pub fn deps(&mut self) -> EvictionDeps<'_> {
        EvictionDeps {
            condenser: &self.condenser,
            salience: &self.salience,
            memory: &mut self.memory,
        }
    }
}

impl Default for TestDeps {
    fn default() -> Self {
        Self::new()
    }
}

/// Shorthand: fresh owned mock deps.
pub fn hooks() -> TestDeps {
    TestDeps::new()
}
