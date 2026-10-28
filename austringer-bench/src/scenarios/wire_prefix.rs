//! Wire-prefix scenario: the PROVIDER-WIRE prompt path under the gate.
//! The prompt scenario measures core's PromptBuilder (the core layout);
//! this one measures the bytes `InferenceAdapter` actually assembles —
//! line-format history + focus-at-tail — across a simulated todo
//! transition (the focus_prefix_identity fixture shape). The stable
//! surface is the common byte prefix of the two assembled prompts; it
//! must cover the entire history region, so focus drifting back to the
//! head fails the scenario outright, and any byte drift in the assembly
//! breaches the committed baseline.

use austringer_core::agent_loop::{FocusChain, TodoId};
use austringer_providers::adapter::{assemble_wire_prompt, render_wire_history};

use super::fixtures::{Rng, synthetic_history};
use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;

pub const HISTORY_EVENTS: u64 = 400;
pub const SEED: u64 = 1117;

/// The volatile region across a todo transition: two todos, the first
/// one marked done between the turns (focus_prefix_identity shapes).
fn focus_pair() -> Result<(String, String), ScenarioError> {
    let mut before = FocusChain::new();
    before.upsert(TodoId(1), "read the map");
    before.upsert(TodoId(2), "ring the bell");
    let mut after = before.clone();
    if !after.mark_done(TodoId(1)) {
        return Err(ScenarioError::Failed(
            "fixture broken: todo 1 must transition open -> done".into(),
        ));
    }
    Ok((before.render(), after.render()))
}

fn once() -> Result<(usize, usize), ScenarioError> {
    let mut rng = Rng::new(SEED);
    let history = synthetic_history(&mut rng, HISTORY_EVENTS);
    let rendered =
        render_wire_history(&history).map_err(|e| ScenarioError::Failed(e.to_string()))?;
    let (focus_before, focus_after) = focus_pair()?;
    if focus_before == focus_after {
        return Err(ScenarioError::Failed(
            "fixture broken: the todo transition must change the focus block".into(),
        ));
    }
    let (prompt_before, _) = assemble_wire_prompt(&rendered, &focus_before);
    let (prompt_after, tail_after) = assemble_wire_prompt(&rendered, &focus_after);
    let shared = prompt_before
        .as_bytes()
        .iter()
        .zip(prompt_after.as_bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let history_region = rendered.len() + 1; // + the join newline
    if shared < history_region {
        return Err(ScenarioError::Failed(format!(
            "todo transition invalidated the wire history prefix: divergence at byte {shared}, \
             history region ends at byte {history_region}"
        )));
    }
    Ok((shared, tail_after))
}

pub fn run(ctx: &ScenarioContext) -> ScenarioResult {
    let mut out = ScenarioOutput::new();
    let mut first: Option<(usize, usize)> = None;
    for _ in 0..ctx.wall_runs.max(1) {
        let result = once()?;
        if let Some(seen) = &first
            && seen != &result
        {
            return Err(ScenarioError::Failed(format!(
                "wire prompt assembly not deterministic across runs: {seen:?} vs {result:?}"
            )));
        }
        first = Some(result);
    }
    let (shared, tail) = first.ok_or_else(|| ScenarioError::Failed("no runs".into()))?;
    out.push(
        "wire.stable_surface_bytes",
        shared as f64,
        "bytes",
        Policy::Regression,
    );
    out.push(
        "wire.focus_tail_bytes",
        tail as f64,
        "bytes",
        Policy::Advisory,
    );
    Ok(out)
}
