//! The deterministic eviction SIM (plan todo 12 acceptance).
//!
//! A seeded generator synthesizes sessions turn by turn (conversation,
//! tool pairs with sized payloads, error observations, an evolving plan);
//! every turn runs the default engine and records a [`TurnReport`].
//! Everything is a pure function of `(config, seed, turns)`: re-running
//! the same session yields an identical report — the stale-state
//! adversarial channel. Deterministic mock seams (no model, no clock, no
//! I/O); the real condenser/salience wiring arrives with todos 13/24.

use async_trait::async_trait;

use super::budget::EvictionInput;
use super::engine::{DefaultEvictionEngine, EvictionDeps, EvictionEngine};
use super::fold::{CondenseError, CondenseRequest, Condenser, ErrorObservation, SalienceFilter};
use super::ladder::StageDecision;
use super::tiers::{Projection, Tier};
use super::{EvictionConfig, EvictionError};
use crate::event::content::ContentBlock;
use crate::event::{Event, RecordedEvent, Sequence};
use crate::eviction::fold::{ReturnCode, ToolReturnTable, VecMemoryHook};
use crate::eviction::ladder::LadderStage;
use crate::eviction::tiers::ActivePlan;

/// Deterministic xorshift64* PRNG (Marsaglia), forced-odd seed.
pub struct SimRng(u64);

impl SimRng {
    /// New generator; the seed is forced odd so state never hits zero.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

struct SimCondenser;

#[async_trait]
impl Condenser for SimCondenser {
    async fn condense(&self, request: CondenseRequest<'_>) -> Result<String, CondenseError> {
        let last = request.span.last().map_or(0, |recorded| recorded.seq.0);
        Ok(format!(
            "sim-condensed {} events through seq {last}",
            request.span.len()
        ))
    }
}

struct FactSalience;

impl SalienceFilter for FactSalience {
    fn salient(&self, event: &RecordedEvent) -> Vec<String> {
        text_blocks(event)
            .iter()
            .flat_map(|text| {
                text.lines()
                    .filter(|line| line.starts_with("FACT:"))
                    .map(|line| (*line).to_owned())
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

fn text_blocks(recorded: &RecordedEvent) -> Vec<&str> {
    let blocks = match &recorded.event {
        Event::User { blocks } | Event::Assistant { blocks } | Event::ToolResult { blocks, .. } => {
            blocks
        }
        _ => return Vec::new(),
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// One simulated turn's decisions and invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnReport {
    /// 1-based turn number.
    pub turn: u64,
    /// Context usage entering the turn.
    pub used_before: u64,
    /// Whether the eviction trigger fired.
    pub triggered: bool,
    /// Whether the pre-compact flush fired.
    pub flush_fired: bool,
    /// Applied ladder stages, in walk order.
    pub applied_stages: Vec<LadderStage>,
    /// Full decision records.
    pub decisions: Vec<StageDecision>,
    /// Usage after eviction.
    pub used_after: u64,
    /// Budget target met.
    pub budget_met: bool,
    /// The active plan survived verbatim.
    pub plan_preserved: bool,
    /// Every folded marker carries the authoritative return code.
    pub tool_codes_intact: bool,
    /// Structural invariants held (protected head/tail verbatim).
    pub invariants_ok: bool,
}

/// The whole simulation: one report per turn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SimReport {
    /// Per-turn reports in order.
    pub turns: Vec<TurnReport>,
}

/// Runs the deterministic eviction SIM over a synthetic session.
///
/// Errors propagate engine failures typed (degenerate inputs, budget
/// unmet, invariant break) — the sim never papers over them.
pub async fn simulate_session(
    config: &EvictionConfig,
    seed: u64,
    turns: u64,
) -> Result<SimReport, EvictionError> {
    let engine = DefaultEvictionEngine::new();
    let mut rng = SimRng::new(seed);
    let mut history: Vec<RecordedEvent> = Vec::new();
    let mut table = ToolReturnTable::default();
    let mut plan = ActivePlan {
        text: "1. simulate\n2. assert invariants".to_owned(),
    };
    let mut last_error: Option<ErrorObservation> = None;
    let mut report = SimReport::default();
    for turn in 1..=turns {
        let mut next_seq = history.last().map_or(1, |last| last.seq.0 + 1);
        history.push(RecordedEvent {
            seq: Sequence(next_seq),
            event: Event::User {
                blocks: vec![ContentBlock::Text {
                    text: format!("turn {turn}: continue the task"),
                }],
            },
        });
        next_seq += 1;
        if rng.below(10) < 6 {
            history.push(RecordedEvent {
                seq: Sequence(next_seq),
                event: Event::Assistant {
                    blocks: vec![ContentBlock::Text {
                        text: format!("assistant working on turn {turn}"),
                    }],
                },
            });
            next_seq += 1;
        }
        if rng.below(10) < 8 {
            let call_id = format!("sim-call-{turn:05}");
            history.push(RecordedEvent {
                seq: Sequence(next_seq),
                event: Event::ToolCall {
                    call_id: call_id.clone(),
                    tool: "execute_command".to_owned(),
                    arguments: serde_json::json!({"cmd": "cargo test"}),
                },
            });
            next_seq += 1;
            let fact = if rng.below(6) == 0 {
                "FACT: metric stable\n"
            } else {
                ""
            };
            // Every tenth turn is a burst: a huge build-log-style output
            // that dominates context pressure until folded (the realistic
            // profile eviction exists for).
            let body = if turn % 10 == 0 {
                "B".repeat(12_000 + rng.below(12_000) as usize)
            } else {
                "y".repeat(600 + rng.below(900) as usize)
            };
            history.push(RecordedEvent {
                seq: Sequence(next_seq),
                event: Event::ToolResult {
                    call_id,
                    blocks: vec![ContentBlock::Text {
                        text: format!("{fact}{body}"),
                    }],
                },
            });
            table.0.insert(
                format!("sim-call-{turn:05}"),
                ReturnCode((turn % 5) as i32 - 1),
            );
        }
        if turn % 17 == 0 {
            last_error = Some(ErrorObservation {
                seq: Sequence(next_seq),
                code: [429, 500, 503][(turn % 3) as usize],
                message: format!("simulated failure at turn {turn}"),
            });
        }
        if turn % 40 == 0 {
            plan.text.push_str(&format!("\nstep {turn} added"));
        }
        let mut input = EvictionInput::new(history.clone())
            .with_plan(plan.clone())
            .with_tool_returns(table.clone());
        input.errors = last_error.clone().into_iter().collect();
        let used_before = input.used_tokens();
        let mut memory = VecMemoryHook::default();
        let outcome = engine
            .evict(
                &input,
                config,
                EvictionDeps {
                    condenser: &SimCondenser,
                    salience: &FactSalience,
                    memory: &mut memory,
                },
            )
            .await?;
        let plan_preserved = outcome.plan_block.as_deref() == Some(plan.render().as_str());
        let tool_codes_intact = codes_intact(&outcome.projected, &table);
        let invariants_ok = protected_intact(&input.history, &outcome.projected, config);
        report.turns.push(TurnReport {
            turn,
            used_before,
            triggered: outcome.triggered,
            flush_fired: outcome.memory_flush.is_some(),
            applied_stages: outcome.applied_stages(),
            decisions: outcome.decisions,
            used_after: outcome.post_tokens,
            budget_met: outcome.budget_met,
            plan_preserved,
            tool_codes_intact,
            invariants_ok,
        });
        history = outcome.projected;
    }
    Ok(report)
}

fn codes_intact(projected: &[RecordedEvent], table: &ToolReturnTable) -> bool {
    projected.iter().all(|recorded| {
        let Event::ToolResult { blocks, .. } = &recorded.event else {
            return true;
        };
        let [ContentBlock::Text { text }] = blocks.as_slice() else {
            return true;
        };
        if !text.starts_with("<folded_tool_output ") {
            return true;
        }
        match crate::eviction::fold::FoldedToolOutput::parse(text) {
            Ok(folded) => table.0.get(&folded.call_id) == Some(&folded.return_code),
            Err(_) => false,
        }
    })
}

fn protected_intact(
    history: &[RecordedEvent],
    projected: &[RecordedEvent],
    config: &EvictionConfig,
) -> bool {
    let projection = Projection::new(history, config);
    (0..history.len()).all(|index| match projection.zone(index) {
        Some(Tier::ProtectedHead) | Some(Tier::ProtectedTail) => {
            let expected = &history[index];
            projected
                .iter()
                .any(|recorded| recorded.seq == expected.seq && recorded.event == expected.event)
        }
        _ => true,
    })
}
