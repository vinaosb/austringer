//! Per-model failure-taxonomy computation from session transcripts (draft
//! amendment T): syntax-error rate, tool-misuse rate, context-overflow
//! frequency, retry storms, edit-rejection rate, and token cost per turn.
//!
//! Computation is LOCAL and pure: observations in, rates out. `Usage`
//! carried on observations must already be NORMALIZED disjoint usage (see
//! [`crate::dialect::normalize_usage`]) so inclusive and exclusive
//! dialects cost identically here.

use austringer_providers::event::Usage;
use austringer_providers::identity::ModelIdentity;
use serde::{Deserialize, Serialize};

/// Minimum observed turns before a taxonomy is reported at all. Below it
/// the honest answer is "no data" — the caller falls back to a neutral
/// profile instead of a noise-shaped one.
pub const MIN_OBSERVED_TURNS: u32 = 10;

/// One failure class observed in a turn (draft amendment T taxonomy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// A produced edit failed syntax validation (pre-flight AST gate).
    SyntaxError,
    /// A tool call was malformed (invalid arguments, unknown tool).
    ToolMisuse,
    /// The provider rejected the request for exceeding the context window.
    ContextOverflow,
    /// Retry/resume budget exhausted on a single turn (retry storm).
    RetryStorm,
    /// An edit was rejected (drifted anchor, ambiguous match, strict-mode
    /// scope violation).
    EditRejection,
}

/// One observed turn, already attributed to the model that served it.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnObservation {
    /// The model that served the turn.
    pub model: ModelIdentity,
    /// Tool calls issued in the turn (misuse denominator).
    pub tool_calls: u32,
    /// Edit-tool calls in the turn (edit-rejection denominator).
    pub edit_calls: u32,
    /// Subagent spawns attempted in the turn (delegation axis input).
    pub subagent_spawns: u32,
    /// Reasoning tokens the provider reported (0 = none reported).
    pub thinking_tokens: u64,
    /// Failures observed in the turn.
    pub failures: Vec<FailureClass>,
    /// Normalized disjoint usage for the turn.
    pub usage: Usage,
}

/// Computed failure taxonomy for one model over an observation window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureTaxonomy {
    /// Turns the taxonomy was computed over.
    pub turns: u32,
    /// Syntax failures per turn.
    pub syntax_error_rate: f32,
    /// Tool-misuse failures per tool call (0 when no calls were made).
    pub tool_misuse_rate: f32,
    /// Context overflows per turn.
    pub context_overflow_rate: f32,
    /// Retry storms per turn.
    pub retry_storm_rate: f32,
    /// Edit rejections per edit call (0 when no edits were attempted).
    pub edit_rejection_rate: f32,
    /// Token cost per turn: normalized cache-miss input + output.
    pub tokens_per_turn: f32,
}

/// Computes the taxonomy for `model` over the transcript. `None` when
/// fewer than [`MIN_OBSERVED_TURNS`] turns belong to the model —
/// insufficient data is reported, never guessed.
#[must_use]
pub fn compute_taxonomy(
    observations: &[TurnObservation],
    model: &ModelIdentity,
) -> Option<FailureTaxonomy> {
    let mine: Vec<&TurnObservation> = observations.iter().filter(|o| o.model == *model).collect();
    let turns = mine.len() as u32;
    if turns < MIN_OBSERVED_TURNS {
        return None;
    }
    let class_count = |class: FailureClass| {
        mine.iter()
            .map(|o| o.failures.iter().filter(|f| **f == class).count() as u64)
            .sum::<u64>()
    };
    let tool_calls: u64 = mine.iter().map(|o| u64::from(o.tool_calls)).sum();
    let edit_calls: u64 = mine.iter().map(|o| u64::from(o.edit_calls)).sum();
    let tokens: u64 = mine
        .iter()
        .map(|o| o.usage.input_tokens + o.usage.output_tokens)
        .sum();
    let per_turn = |count: u64| count as f32 / turns as f32;
    Some(FailureTaxonomy {
        turns,
        syntax_error_rate: per_turn(class_count(FailureClass::SyntaxError)),
        tool_misuse_rate: class_count(FailureClass::ToolMisuse) as f32 / tool_calls.max(1) as f32,
        context_overflow_rate: per_turn(class_count(FailureClass::ContextOverflow)),
        retry_storm_rate: per_turn(class_count(FailureClass::RetryStorm)),
        edit_rejection_rate: class_count(FailureClass::EditRejection) as f32
            / edit_calls.max(1) as f32,
        tokens_per_turn: tokens as f32 / turns as f32,
    })
}
