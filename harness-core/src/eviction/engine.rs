//! Pluggable eviction engines (plan todo 12).
//!
//! [`EvictionEngine`] is the one-trait seam; [`DefaultEvictionEngine`]
//! implements the full doctrine — pre-compact flush, ladder walk until
//! the target budget is met, Tier0 and protected head/tail structurally
//! untouchable, folding never drops return codes — and
//! [`MinimalEngine`] is a second, truncation-only engine proving the
//! trait is pluggable.

use async_trait::async_trait;

use super::budget::EvictionInput;
use super::fold::{Condenser, ErrorObservation, MemoryFlush, MemoryHook, SalienceFilter};
use super::ladder::{LadderStage, StageDecision};
use super::tiers::{ActivePlan, Projection, Tier};
use super::walk::ReplaceCtx;
use super::{EvictionConfig, EvictionError};
use crate::event::{ContentBlock, Event, RecordedEvent};

/// The eviction result feeding the `[L1,L2)` history composition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvictionOutcome {
    /// Whether the eviction trigger fired this turn.
    pub triggered: bool,
    /// Whether post-eviction usage meets the target budget.
    pub budget_met: bool,
    /// Post-eviction usage estimate.
    pub post_tokens: u64,
    /// Applied-stage decisions in walk order.
    pub decisions: Vec<StageDecision>,
    /// Condensates produced by the distillation stage.
    pub condensates: Vec<super::fold::CondensateBlock>,
    /// The pre-compact salient-fact flush, when it fired.
    pub memory_flush: Option<MemoryFlush>,
    /// The single volatile error slot (the latest observation).
    pub error_slot: Option<ErrorObservation>,
    /// The Tier0 plan render, carried verbatim (never evicted).
    pub plan_block: Option<String>,
    /// The projected history: what the prompt builder renders.
    pub projected: Vec<RecordedEvent>,
}

impl EvictionOutcome {
    /// The projected history slice.
    #[must_use]
    pub fn projected_history(&self) -> &[RecordedEvent] {
        &self.projected
    }

    /// The applied stages in walk order.
    #[must_use]
    pub fn applied_stages(&self) -> Vec<LadderStage> {
        self.decisions
            .iter()
            .map(|decision| decision.stage)
            .collect()
    }
}

/// The injected seams one eviction turn may call.
pub struct EvictionDeps<'a> {
    /// Cheap-model condensation seam.
    pub condenser: &'a dyn Condenser,
    /// Salience seam for the pre-compact flush.
    pub salience: &'a dyn SalienceFilter,
    /// Memory-store hook.
    pub memory: &'a mut dyn MemoryHook,
}

/// The pluggable eviction engine seam.
#[async_trait]
pub trait EvictionEngine: Send + Sync {
    /// Projects one turn's history under `config`, evicting as the
    /// engine's policy dictates.
    async fn evict(
        &self,
        input: &EvictionInput,
        config: &EvictionConfig,
        deps: EvictionDeps<'_>,
    ) -> Result<EvictionOutcome, EvictionError>;
}

/// The doctrine engine (see module docs).
#[derive(Debug, Default)]
pub struct DefaultEvictionEngine;

impl DefaultEvictionEngine {
    /// A fresh engine.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EvictionEngine for DefaultEvictionEngine {
    async fn evict(
        &self,
        input: &EvictionInput,
        config: &EvictionConfig,
        mut deps: EvictionDeps<'_>,
    ) -> Result<EvictionOutcome, EvictionError> {
        let (usage, error_slot, plan_block) = prelude(input, config)?;
        let memory_flush = flush_if_due(usage, config, input, &mut deps);
        if usage < config.budget.trigger_tokens() {
            return Ok(passthrough(
                usage,
                error_slot,
                plan_block,
                memory_flush,
                input,
            ));
        }
        let mut ctx = ReplaceCtx::new(input, config);
        // Stage eligibility compares the walk's STARTING usage: a stage's
        // trigger percent says when the walk may escalate TO it, not that
        // usage must still sit above the bar mid-walk (early-stage
        // reclaim would otherwise lock out the stronger stages and stall
        // the walk above target).
        let walk_start_usage = ctx.usage();
        for (stage, stage_config) in &config.ladder.stages {
            if ctx.usage() <= config.budget.target_tokens() {
                break;
            }
            let threshold = config
                .budget
                .window_tokens
                .saturating_mul(stage_config.trigger_percent)
                / 100;
            if walk_start_usage < threshold {
                continue;
            }
            let decision = match stage {
                LadderStage::Masking => ctx.mask(stage_config.budget_tokens)?,
                LadderStage::Distillation => {
                    ctx.distill(stage_config.budget_tokens, deps.condenser)
                        .await?
                }
                LadderStage::Compression => ctx.compress(stage_config.budget_tokens, config)?,
                LadderStage::Truncation => ctx.truncate(stage_config.budget_tokens)?,
            };
            if decision.items > 0 {
                ctx.decisions.push(decision);
            }
        }
        finish(ctx, input, config, error_slot, plan_block, memory_flush)
    }
}

/// The minimal engine: truncation-only, no ladder, no flush — the second
/// implementation proving [`EvictionEngine`] is pluggable.
#[derive(Debug)]
pub struct MinimalEngine;

#[async_trait]
impl EvictionEngine for MinimalEngine {
    async fn evict(
        &self,
        input: &EvictionInput,
        config: &EvictionConfig,
        deps: EvictionDeps<'_>,
    ) -> Result<EvictionOutcome, EvictionError> {
        let (usage, error_slot, plan_block) = prelude(input, config)?;
        if usage < config.budget.trigger_tokens() {
            return Ok(passthrough(usage, error_slot, plan_block, None, input));
        }
        let _ = deps;
        let mut ctx = ReplaceCtx::new(input, config);
        let decision = ctx.truncate(u64::MAX)?;
        if decision.items > 0 {
            ctx.decisions.push(decision);
        }
        finish(ctx, input, config, error_slot, plan_block, None)
    }
}

type Prelude = (u64, Option<ErrorObservation>, Option<String>);

fn prelude(input: &EvictionInput, config: &EvictionConfig) -> Result<Prelude, EvictionError> {
    if let Some(degenerate) = Projection::degenerate_of(&input.history, config) {
        return Err(EvictionError::Degenerate(degenerate));
    }
    Ok((
        input.used_tokens(),
        input.latest_error().cloned(),
        input.plan.as_ref().map(ActivePlan::render),
    ))
}

// `passthrough`/`finish` thread five inputs that map 1:1 onto outcome
// fields through two assembly paths; clippy's threshold is not hit and a
// wrapper struct would only relocate the same data.
fn passthrough(
    usage: u64,
    error_slot: Option<ErrorObservation>,
    plan_block: Option<String>,
    memory_flush: Option<MemoryFlush>,
    input: &EvictionInput,
) -> EvictionOutcome {
    EvictionOutcome {
        triggered: false,
        budget_met: true,
        post_tokens: usage,
        decisions: Vec::new(),
        condensates: Vec::new(),
        memory_flush,
        error_slot,
        plan_block,
        projected: input.history.clone(),
    }
}

fn flush_if_due(
    usage: u64,
    config: &EvictionConfig,
    input: &EvictionInput,
    deps: &mut EvictionDeps<'_>,
) -> Option<MemoryFlush> {
    if usage < config.budget.flush_threshold_tokens() {
        return None;
    }
    let facts: Vec<String> = input
        .history
        .iter()
        .flat_map(|recorded| deps.salience.salient(recorded))
        .collect();
    if facts.is_empty() {
        return None;
    }
    deps.memory.flush(facts.clone());
    Some(MemoryFlush { facts })
}

fn finish(
    ctx: ReplaceCtx<'_>,
    input: &EvictionInput,
    config: &EvictionConfig,
    error_slot: Option<ErrorObservation>,
    plan_block: Option<String>,
    memory_flush: Option<MemoryFlush>,
) -> Result<EvictionOutcome, EvictionError> {
    let post = ctx.usage();
    let target = config.budget.target_tokens();
    if post > target {
        return Err(EvictionError::BudgetUnmet {
            usage: post,
            target,
        });
    }
    verify(&ctx.working, input, config)?;
    Ok(EvictionOutcome {
        triggered: true,
        budget_met: true,
        post_tokens: post,
        decisions: ctx.decisions,
        condensates: ctx.condensates,
        memory_flush,
        error_slot,
        plan_block,
        projected: ctx.working,
    })
}

/// Post-walk invariant gate: protected head/tail events survive verbatim
/// and every folded marker carries the authoritative return code.
fn verify(
    projected: &[RecordedEvent],
    input: &EvictionInput,
    config: &EvictionConfig,
) -> Result<(), EvictionError> {
    let projection = Projection::new(&input.history, config);
    for index in 0..input.history.len() {
        if matches!(
            projection.zone(index),
            Some(Tier::ProtectedHead) | Some(Tier::ProtectedTail)
        ) {
            let expected = &input.history[index];
            let survived = projected
                .iter()
                .any(|recorded| recorded.seq == expected.seq && recorded.event == expected.event);
            if !survived {
                return Err(EvictionError::InvariantBroken {
                    detail: format!("protected event {} was evicted", expected.seq.0),
                });
            }
        }
    }
    for recorded in projected {
        if let Event::ToolResult { blocks, .. } = &recorded.event
            && let [ContentBlock::Text { text }] = blocks.as_slice()
            && text.starts_with("<folded_tool_output ")
        {
            let folded = super::fold::FoldedToolOutput::parse(text).map_err(|error| {
                EvictionError::InvariantBroken {
                    detail: error.to_string(),
                }
            })?;
            let authoritative = input.tool_returns.0.get(&folded.call_id);
            if authoritative != Some(&folded.return_code) {
                return Err(EvictionError::InvariantBroken {
                    detail: format!("return code dropped for {}", folded.call_id),
                });
            }
        }
    }
    Ok(())
}
