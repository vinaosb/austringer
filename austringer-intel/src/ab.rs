//! A/B skill-variant optimization (draft amendment T): variant outcomes
//! from the evals adapter (injected scores — no live-model calls ever
//! happen here) select the best variant PER MODEL deterministically, and
//! the winners are rendered as Skill Forge proposals. Proposals are
//! DRAFT-tier by construction: they flow through the todo-31 trust gates
//! (scan -> approval -> replay eval) and this crate has no activation
//! path at all.

use std::collections::BTreeMap;

use austringer_skills::forge::distill::DistillProposal;
use austringer_skills::forge::document::{SkillSection, SkillSpec};

/// One variant's eval outcome on one model (mean score + sample count
/// from the evals adapter).
#[derive(Debug, Clone, PartialEq)]
pub struct VariantOutcome {
    /// Variant identifier (e.g. a skill-guidance variant name).
    pub variant: String,
    /// Model the variant ran on.
    pub model: String,
    /// Mean score across samples.
    pub score: f64,
    /// Samples the mean was computed over. Zero-sample outcomes are
    /// excluded from selection — an unscored variant is not evidence.
    pub samples: u32,
}

/// The runner-up evidence for one model.
#[derive(Debug, Clone, PartialEq)]
pub struct RunnerUp {
    /// Runner-up variant id.
    pub variant: String,
    /// Its mean score.
    pub score: f64,
}

/// The selected winner for one model.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelWinner {
    /// Model the selection is for.
    pub model: String,
    /// Winning variant id.
    pub winner: String,
    /// Winner's mean score.
    pub winner_score: f64,
    /// The runner-up, when at least two variants ran.
    pub runner_up: Option<RunnerUp>,
    /// Samples behind the winning score.
    pub samples: u32,
}

/// The full A/B decision: one winner per model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AbDecision {
    /// Winners, sorted by model id (deterministic order).
    pub winners: Vec<ModelWinner>,
}

/// Selects the winning variant per model. Deterministic by construction:
/// candidates sort by score DESCENDING (total order via `f64::total_cmp`)
/// with variant id ASCENDING as the tie-break, so identical inputs always
/// produce the identical decision.
#[must_use]
pub fn ab_select(outcomes: &[VariantOutcome]) -> AbDecision {
    let mut by_model: BTreeMap<&str, Vec<&VariantOutcome>> = BTreeMap::new();
    for outcome in outcomes.iter().filter(|o| o.samples > 0) {
        by_model
            .entry(outcome.model.as_str())
            .or_default()
            .push(outcome);
    }
    let winners = by_model
        .into_iter()
        .map(|(model, mut candidates)| {
            candidates.sort_by(|a, b| {
                b.score
                    .total_cmp(&a.score)
                    .then_with(|| a.variant.cmp(&b.variant))
            });
            let winner = candidates[0];
            let runner_up = candidates.get(1).map(|ru| RunnerUp {
                variant: ru.variant.clone(),
                score: ru.score,
            });
            ModelWinner {
                model: model.to_string(),
                winner: winner.variant.clone(),
                winner_score: winner.score,
                runner_up,
                samples: winner.samples,
            }
        })
        .collect();
    AbDecision { winners }
}

/// Renders one Skill Forge proposal per winning model: the skill's
/// per-model guidance section pinned to the winning variant, with the
/// numeric evidence in the rationale shown to the approver.
#[must_use]
pub fn winner_proposals(decision: &AbDecision, base: &SkillSpec) -> Vec<DistillProposal> {
    decision
        .winners
        .iter()
        .map(|w| {
            let versus = match &w.runner_up {
                Some(ru) => format!(" vs runner-up `{}` at {:.4}", ru.variant, ru.score),
                None => String::new(),
            };
            DistillProposal {
                spec: base.clone(),
                sections: vec![SkillSection {
                    heading: "Per-model guidance".to_string(),
                    body: format!(
                        "with {}: prefer variant `{}` (A/B winner)",
                        w.model, w.winner
                    ),
                }],
                rationale: format!(
                    "A/B variant selection (todo 34): variant `{}` won on {} with \
                     mean {:.4} over {} samples{}.",
                    w.winner, w.model, w.winner_score, w.samples, versus
                ),
            }
        })
        .collect()
}
