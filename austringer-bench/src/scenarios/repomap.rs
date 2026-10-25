//! Repomap scenario: full build -> rank -> budget-render pipeline over the
//! deterministic synthetic repo (todo 15 fixture). Tokens is the
//! deterministic gate metric; full-pipeline wall is advisory.

use std::time::Instant;

use austringer_repomap::{MapOptions, repo_map, synthetic};

use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;
use crate::stats::min_of;

pub const FILES: usize = 2_000;
pub const SEED: u64 = 43;

fn once() -> Result<(u32, String), ScenarioError> {
    let tables = synthetic::repo(FILES, SEED);
    let output = repo_map(tables, &MapOptions::default());
    Ok((output.tokens, output.text))
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let mut out = ScenarioOutput::new();
    let mut walls = Vec::new();
    let mut first: Option<(u32, String)> = None;
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
                "repomap output not deterministic across runs: tokens {} vs {}",
                seen.0, result.0
            )));
        }
        first = Some(result);
    }
    let (tokens, text) = first.ok_or_else(|| ScenarioError::Failed("no runs".into()))?;
    out.push(
        "repomap.tokens",
        f64::from(tokens),
        "tokens",
        Policy::Regression,
    );
    out.push(
        "repomap.map_bytes",
        text.len() as f64,
        "bytes",
        Policy::Advisory,
    );
    out.push(
        "repomap.full_ms",
        min_of(&walls)
            .map_err(|e| ScenarioError::Failed(e.to_string()))?
            .as_secs_f64()
            * 1000.0,
        "ms",
        Policy::Advisory,
    );
    #[cfg(feature = "dhat")]
    super::heap::record(&mut out, "repomap", heap_before);
    Ok(out)
}
