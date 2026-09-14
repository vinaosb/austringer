//! PTY scrape scenario: seeded VT byte stream through the todo-19
//! screen-delta model + the 25/50 window law — fully synthetic, no real
//! PTY. Bounded observation bytes is the deterministic gate metric.

use std::time::Instant;

use harness_exec::screen::ScreenModel;
use harness_exec::window::window_lines;

use super::fixtures::Rng;
use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;
use crate::stats::min_of;

pub const LINES: u64 = 300;
pub const SEED: u64 = 1979;

/// Deterministic VT stream: colored runs, cursor jumps, wrapped rows.
pub fn vt_stream(rng: &mut Rng) -> Vec<u8> {
    let mut bytes = Vec::new();
    for i in 0..LINES {
        let color = 31 + (rng.below(7));
        bytes.extend_from_slice(format!("\x1b[{color}m").as_bytes());
        bytes.extend_from_slice(format!("line {i}: ").as_bytes());
        bytes.extend_from_slice(rng.text(24).as_bytes());
        bytes.extend_from_slice(b"\x1b[0m\r\n");
    }
    bytes
}

fn once() -> Result<(usize, u64), ScenarioError> {
    let mut rng = Rng::new(SEED);
    let stream = vt_stream(&mut rng);
    let mut model = ScreenModel::new(120, 40);
    // Chunked like a real PTY emitter; the model is chunk-invariant
    // (pinned by harness-exec's own suite).
    for chunk in stream.chunks(4096) {
        model.process(chunk);
    }
    let view = model.finish();
    let lines = view.transcript_lines();
    let total = view.scrolled_total + lines.len() as u64;
    let windowed = window_lines(&lines, total);
    Ok((windowed.text.len(), windowed.omitted_lines))
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let mut out = ScenarioOutput::new();
    let mut walls = Vec::new();
    let mut first: Option<(usize, u64)> = None;
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
                "pty scrape not deterministic across runs: {seen:?} vs {result:?}"
            )));
        }
        first = Some(result);
    }
    let (observation, omitted) = first.ok_or_else(|| ScenarioError::Failed("no runs".into()))?;
    out.push(
        "pty.observation_bytes",
        observation as f64,
        "bytes",
        Policy::Regression,
    );
    out.push(
        "pty.omitted_lines",
        omitted as f64,
        "lines",
        Policy::Advisory,
    );
    out.push(
        "pty.scrape_ms",
        min_of(&walls)
            .map_err(|e| ScenarioError::Failed(e.to_string()))?
            .as_secs_f64()
            * 1000.0,
        "ms",
        Policy::Advisory,
    );
    #[cfg(feature = "dhat")]
    super::heap::record(&mut out, "pty", heap_before);
    Ok(out)
}
