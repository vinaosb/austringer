//! Skill A/B mini-benchmarks (todo 44; the todo-34 consumer). Runs a
//! dataset through every skill VARIANT's subject on one model and
//! emits per-variant outcome rows in EXACTLY the shape
//! `harness-intel/src/ab.rs::VariantOutcome` consumes — fields
//! `variant` (String), `model` (String), `score` (f64), `samples`
//! (u32) — so the evals adapter feeds the A/B selector without an
//! adapter layer. Selection mirrors intel's `ab_select` total order
//! (score DESC via `total_cmp`, variant id ASC) — deterministic, and
//! the pick here must always agree with intel's.

use std::collections::BTreeMap;

use crate::dataset::EvalDataset;
use crate::guard::GuardSet;
use crate::metrics::pair_metrics;
use crate::runner::{Subject, SweepPair, run_pair};

/// One variant's outcome row (the todo-34 `VariantOutcome` shape).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VariantOutcomeRow {
    /// Variant identifier.
    pub variant: String,
    /// Model the variant ran on.
    pub model: String,
    /// Mean guarded score.
    pub score: f64,
    /// Samples the mean was computed over (zero-sample rows are
    /// excluded downstream — an unscored variant is not evidence).
    pub samples: u32,
}

/// The pinned field names of the A/B feeder row (doc-constant).
pub const VARIANT_OUTCOME_FIELDS: [&str; 4] = ["variant", "model", "score", "samples"];

/// The picked winner (mirrors the intel selection).
#[derive(Debug, Clone, PartialEq)]
pub struct AbWinner {
    /// Winning variant id.
    pub variant: String,
    /// Its mean score.
    pub score: f64,
}

/// Runs every variant as a one-rep "pair" over the same dataset
/// (variant name rides the profile slot) and emits outcome rows,
/// sorted by variant id (deterministic).
#[must_use]
pub fn run_variant_bench(
    model: &str,
    dataset: &EvalDataset,
    variants: &mut BTreeMap<String, &mut dyn Subject>,
    guards: &GuardSet,
) -> Vec<VariantOutcomeRow> {
    variants
        .iter_mut()
        .map(|(variant, subject)| {
            let pair = SweepPair {
                model: model.to_owned(),
                profile: variant.clone(),
            };
            let run = run_pair(&pair, dataset, 1, *subject, guards);
            let metrics = pair_metrics(&run.cases);
            VariantOutcomeRow {
                variant: variant.clone(),
                model: model.to_owned(),
                score: metrics.mean,
                samples: run.cases.iter().map(|case| case.reps.len() as u32).sum(),
            }
        })
        .collect()
}

/// Picks the winner with intel's exact total order (score DESC,
/// variant ASC). Zero-sample rows are not evidence.
#[must_use]
pub fn pick_winner(rows: &[VariantOutcomeRow]) -> Option<AbWinner> {
    let mut candidates: Vec<&VariantOutcomeRow> =
        rows.iter().filter(|row| row.samples > 0).collect();
    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.variant.cmp(&b.variant))
    });
    candidates.first().map(|row| AbWinner {
        variant: row.variant.clone(),
        score: row.score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{EvalCase, EvalDataset};
    use crate::grader::GraderSpec;
    use crate::runner::ScriptedSubject;

    fn dataset() -> EvalDataset {
        EvalDataset {
            cases: vec![EvalCase {
                id: "a".to_owned(),
                prompt: "p".to_owned(),
                grader: GraderSpec::AllOf {
                    expected: vec!["good".to_owned()],
                },
                threshold: 1.0,
                canary: None,
                tags: vec![],
            }],
        }
    }

    fn lines(text: &str) -> Vec<String> {
        vec![
            serde_json::json!({
                "event": { "id": 1, "event": {
                    "type": "assistant",
                    "blocks": [ { "type": "text", "text": text } ]
                }}
            })
            .to_string(),
            r#"{"type":"turn_result","ok":true,"outcome":"task_completed","turn":1}"#.to_owned(),
        ]
    }

    fn subject_for(text: &str) -> ScriptedSubject {
        let mut subject = ScriptedSubject::new();
        subject.add("a", vec![lines(text)]);
        subject
    }

    #[test]
    fn ab_bench_picks_the_better_variant_deterministically() {
        let dataset = dataset();
        let guards = GuardSet::standard();
        let mut good = subject_for("good answer");
        let mut bad = subject_for("wrong answer");
        let mut variants: BTreeMap<String, &mut dyn Subject> = BTreeMap::new();
        variants.insert("guidance-v2".to_owned(), &mut good);
        variants.insert("guidance-v1".to_owned(), &mut bad);
        let rows = run_variant_bench("mini", &dataset, &mut variants, &guards);
        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert_eq!(row.model, "mini");
            assert_eq!(row.samples, 1);
        }
        let winner = pick_winner(&rows).unwrap_or_else(|| panic!("no winner"));
        assert_eq!(winner.variant, "guidance-v2");
        assert!((winner.score - 1.0).abs() < 1e-9);
    }

    /// DOC-CONSTANT SYNC TEST: the row shape is exactly todo-34's
    /// `VariantOutcome` field names.
    #[test]
    fn serialized_rows_match_the_intel_variant_outcome_contract() {
        let row = VariantOutcomeRow {
            variant: "v".to_owned(),
            model: "m".to_owned(),
            score: 0.5,
            samples: 3,
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&row).unwrap_or_default())
                .unwrap_or_default();
        let mut keys: Vec<&str> = json
            .as_object()
            .map_or(Vec::new(), |m| m.keys().map(String::as_str).collect());
        keys.sort_unstable();
        let mut expected: Vec<&str> = VARIANT_OUTCOME_FIELDS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected);
    }

    #[test]
    fn zero_sample_rows_are_not_evidence() {
        let rows = vec![VariantOutcomeRow {
            variant: "v".to_owned(),
            model: "m".to_owned(),
            score: 0.0,
            samples: 0,
        }];
        assert_eq!(pick_winner(&rows), None);
    }

    #[test]
    fn variant_pair_shape_is_sweep_compatible() {
        // sanity: variant benches ride the same SweepPair machinery.
        let pair = SweepPair {
            model: "mini".to_owned(),
            profile: "guidance-v2".to_owned(),
        };
        assert_eq!(pair.label(), "mini/guidance-v2");
        let _ = run_pair;
    }
}
