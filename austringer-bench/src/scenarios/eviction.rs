//! Eviction scenario: one full tiered-eviction pass (todo 12 engine) over
//! a seeded synthetic history past the trigger. Projected-event count is
//! the deterministic gate metric; evict wall is advisory.

use std::time::Instant;

use austringer_core::eviction::{
    ActivePlan, DefaultEvictionEngine, EvictionConfig, EvictionDeps, EvictionEngine, EvictionInput,
    VecMemoryHook,
};

use super::fixtures::{FixedCondenser, NoSalience, Rng, bloated_tool_history, tool_returns};
use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;
use crate::stats::min_of;

pub const PAIRS: u64 = 260;
/// Window sized so the post-folding residual (call args + markers +
/// protected zones) sits well under the 60% target — the walk's own
/// ceiling, not a tuned-to-the-edge number.
pub const WINDOW_TOKENS: u64 = 80_000;
pub const SEED: u64 = 2026_0943;

fn once(runtime: &tokio::runtime::Runtime) -> Result<(usize, u64), ScenarioError> {
    let mut rng = Rng::new(SEED);
    let history = bloated_tool_history(&mut rng, PAIRS);
    let config = EvictionConfig::default().with_window(WINDOW_TOKENS);
    let input = EvictionInput::new(history.clone())
        .with_plan(ActivePlan {
            text: "1. land perf gate 2. record baselines".to_owned(),
        })
        .with_tool_returns(tool_returns(&history))
        .with_overhead(0);
    let engine = DefaultEvictionEngine::new();
    let mut condenser = FixedCondenser;
    let mut salience = NoSalience;
    let mut memory = VecMemoryHook::default();
    let started = Instant::now();
    let outcome = runtime
        .block_on(engine.evict(
            &input,
            &config,
            EvictionDeps {
                condenser: &mut condenser,
                salience: &mut salience,
                memory: &mut memory,
            },
        ))
        .map_err(|e| ScenarioError::Failed(e.to_string()))?;
    Ok((
        outcome.projected_history().len(),
        started.elapsed().as_micros() as u64,
    ))
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let runtime =
        tokio::runtime::Runtime::new().map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let mut out = ScenarioOutput::new();
    let mut projected: Option<usize> = None;
    let mut walls = Vec::new();
    #[cfg(feature = "dhat")]
    let heap_before = super::heap::snapshot();
    for _ in 0..ctx.wall_runs.max(1) {
        let (events, micros) = once(&runtime)?;
        if let Some(seen) = projected
            && seen != events
        {
            return Err(ScenarioError::Failed(format!(
                "eviction projection not deterministic across runs: {seen} vs {events}"
            )));
        }
        projected = Some(events);
        walls.push(std::time::Duration::from_micros(micros));
    }
    let events = projected.ok_or_else(|| ScenarioError::Failed("no runs".into()))?;
    out.push(
        "eviction.projected_events",
        events as f64,
        "events",
        Policy::Regression,
    );
    out.push(
        "eviction.evict_ms",
        min_of(&walls)
            .map_err(|e| ScenarioError::Failed(e.to_string()))?
            .as_secs_f64()
            * 1000.0,
        "ms",
        Policy::Advisory,
    );
    #[cfg(feature = "dhat")]
    super::heap::record(&mut out, "eviction", heap_before);
    Ok(out)
}
