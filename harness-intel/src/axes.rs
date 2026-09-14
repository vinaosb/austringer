//! Behavioral axes (draft amendment T, research `.omo/research/
//! model-profiles.md` SS1): the per-model behavior vocabulary tracked from
//! transcripts — verbosity, delegation preference, tool-call precision,
//! edit-format competence, instruction-drift curve, thinking depth, and
//! tool volume. The elicitation layer gates PROMPT counterweights on
//! these observed values ("gated on OUR evals", research SS33.6).

use harness_providers::identity::ModelIdentity;
use serde::{Deserialize, Serialize};

use super::taxonomy::{FailureClass, MIN_OBSERVED_TURNS, TurnObservation};

/// Observed behavioral axes for one model over an observation window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BehaviorAxes {
    /// Output tokens per turn (A5 verbosity index).
    pub verbosity_tokens_per_turn: f32,
    /// Tool calls per turn (over-exploration signal, A1/A6 adjacent).
    pub tool_calls_per_turn: f32,
    /// Subagent spawns per turn (A6 delegation preference; 0 = never
    /// delegates).
    pub delegation_ratio: f32,
    /// 1 - tool-misuse rate (A7-adjacent tool-call precision), clamped to
    /// [0, 1].
    pub tool_precision: f32,
    /// 1 - edit-rejection rate (A7 edit-format competence), clamped to
    /// [0, 1].
    pub edit_competence: f32,
    /// Failure-rate delta between the second and first half of the window
    /// (A8 instruction drift; positive = degrading late, negative =
    /// improving).
    pub drift_delta: f32,
    /// Mean reasoning tokens over the turns that reported any; `None` when
    /// the model reported none (A4 thinking depth).
    pub thinking_tokens_per_turn: Option<f32>,
}

/// Computes the axes for `model` over the transcript. `None` under the
/// [`MIN_OBSERVED_TURNS`] floor, matching the taxonomy gate.
#[must_use]
pub fn compute_axes(
    observations: &[TurnObservation],
    model: &ModelIdentity,
) -> Option<BehaviorAxes> {
    let mine: Vec<&TurnObservation> = observations.iter().filter(|o| o.model == *model).collect();
    let turns = mine.len() as u32;
    if turns < MIN_OBSERVED_TURNS {
        return None;
    }
    let turns_f = turns as f32;
    let failures_of = |class: FailureClass, slice: &[&TurnObservation]| -> u32 {
        slice
            .iter()
            .map(|o| o.failures.iter().filter(|f| **f == class).count() as u32)
            .sum()
    };

    let tool_calls: u32 = mine.iter().map(|o| o.tool_calls).sum();
    let edit_calls: u32 = mine.iter().map(|o| o.edit_calls).sum();
    let output_tokens: u64 = mine.iter().map(|o| o.usage.output_tokens).sum();
    let spawns: u32 = mine.iter().map(|o| o.subagent_spawns).sum();
    let misuse = failures_of(FailureClass::ToolMisuse, &mine);
    let rejections = failures_of(FailureClass::EditRejection, &mine);

    // Drift: any-failure rate per turn, second half minus first half.
    let half = mine.len() / 2;
    let rate = |slice: &[&TurnObservation]| -> f32 {
        if slice.is_empty() {
            0.0
        } else {
            let all: u32 = slice.iter().map(|o| o.failures.len() as u32).sum();
            all as f32 / slice.len() as f32
        }
    };
    let (early, late) = mine.split_at(half);

    let thinking: Vec<u64> = mine
        .iter()
        .map(|o| o.thinking_tokens)
        .filter(|t| *t > 0)
        .collect();

    Some(BehaviorAxes {
        verbosity_tokens_per_turn: output_tokens as f32 / turns_f,
        tool_calls_per_turn: tool_calls as f32 / turns_f,
        delegation_ratio: spawns as f32 / turns_f,
        tool_precision: (1.0 - misuse as f32 / tool_calls.max(1) as f32).clamp(0.0, 1.0),
        edit_competence: (1.0 - rejections as f32 / edit_calls.max(1) as f32).clamp(0.0, 1.0),
        drift_delta: rate(late) - rate(early),
        thinking_tokens_per_turn: (!thinking.is_empty())
            .then(|| thinking.iter().sum::<u64>() as f32 / thinking.len() as f32),
    })
}
