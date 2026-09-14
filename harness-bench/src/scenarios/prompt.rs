//! Prompt-builder scenario: T11 prefix-cache-stable render over a seeded
//! synthetic session. Stable-prefix bytes (and total render bytes) are
//! deterministic gate metrics; render wall is advisory.

use std::time::Instant;

use harness_core::context::PromptBuilder;

use super::fixtures::{Rng, context_spec, synthetic_history};
use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;
use crate::stats::min_of;

pub const HISTORY_EVENTS: u64 = 400;
pub const SEED: u64 = 1109;

fn once() -> Result<(usize, usize), ScenarioError> {
    let mut rng = Rng::new(SEED);
    let history = synthetic_history(&mut rng, HISTORY_EVENTS);
    let spec = context_spec(history);
    let rendered =
        PromptBuilder::render(&spec).map_err(|e| ScenarioError::Failed(e.to_string()))?;
    Ok((rendered.stable_prefix_bytes(), rendered.prompt.len()))
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let mut out = ScenarioOutput::new();
    let mut walls = Vec::new();
    let mut first: Option<(usize, usize)> = None;
    #[cfg(feature = "dhat")]
    let heap_before = super::heap::snapshot();
    for _ in 0..ctx.wall_runs.max(1) {
        let started = Instant::now();
        let result = once()?;
        walls.push(started.elapsed());
        if let Some(seen) = &first
            && seen != &result
        {
            return Err(ScenarioError::Failed(format!(
                "prompt render not deterministic across runs: {seen:?} vs {result:?}"
            )));
        }
        first = Some(result);
    }
    let (prefix, total) = first.ok_or_else(|| ScenarioError::Failed("no runs".into()))?;
    out.push(
        "prompt.stable_prefix_bytes",
        prefix as f64,
        "bytes",
        Policy::Regression,
    );
    out.push(
        "prompt.total_bytes",
        total as f64,
        "bytes",
        Policy::Advisory,
    );
    out.push(
        "prompt.build_ms",
        min_of(&walls)
            .map_err(|e| ScenarioError::Failed(e.to_string()))?
            .as_secs_f64()
            * 1000.0,
        "ms",
        Policy::Advisory,
    );
    #[cfg(feature = "dhat")]
    super::heap::record(&mut out, "prompt", heap_before);
    Ok(out)
}
