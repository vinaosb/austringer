//! Unit tests for the metrics surface (Pass^k floors, fingerprints,
//! ablation, coverage) — moved out of `src/metrics.rs` to keep the
//! source file under the 250-pure-LOC ceiling.

use harness_evals::guard::IntegrityFlag;
use harness_evals::metrics::{self, CaseMetrics, FailureCategory, RepResult};

fn rep(
    score: f64,
    passed: bool,
    category: FailureCategory,
    guard: Option<(&str, IntegrityFlag)>,
) -> RepResult {
    RepResult {
        rep: 1,
        raw_score: score,
        score,
        passed,
        category,
        hits: guard
            .map(|(id, flag)| (id.to_owned(), flag))
            .into_iter()
            .collect(),
        tokens: Some(100),
    }
}

#[test]
fn pass_k_floor_requires_every_rep_to_pass() {
    let all_pass = vec![
        rep(1.0, true, FailureCategory::Pass, None),
        rep(1.0, true, FailureCategory::Pass, None),
        rep(1.0, true, FailureCategory::Pass, None),
    ];
    let metrics = metrics::case_metrics("case", 1.0, all_pass);
    assert!(metrics.pass_k);
    assert_eq!(metrics.mean_at_k, 1.0);
    let flaky = vec![
        rep(1.0, true, FailureCategory::Pass, None),
        rep(0.0, false, FailureCategory::GraderMismatch, None),
        rep(1.0, true, FailureCategory::Pass, None),
    ];
    let flaky_metrics = metrics::case_metrics("case", 1.0, flaky);
    assert!(!flaky_metrics.pass_k);
    assert!((flaky_metrics.mean_at_k - (2.0 / 3.0)).abs() < 1e-9);
}

#[test]
fn pair_metrics_fingerprint_and_tokens_solved() {
    let cases = vec![
        metrics::case_metrics("a", 1.0, vec![rep(1.0, true, FailureCategory::Pass, None)]),
        metrics::case_metrics(
            "b",
            1.0,
            vec![rep(
                0.0,
                false,
                FailureCategory::TurnFailed,
                Some(("turn_outcome", IntegrityFlag::TurnFailed)),
            )],
        ),
    ];
    let pair = metrics::pair_metrics(&cases);
    assert!((pair.pass_k - 0.5).abs() < 1e-9);
    assert_eq!(pair.fingerprint.get("pass"), Some(&1));
    assert_eq!(pair.fingerprint.get("turn_failed"), Some(&1));
    // tokens_solved: only the solved case counts.
    assert_eq!(pair.tokens_solved, Some(100));
}

#[test]
fn leave_one_out_ablation_recovers_guard_zeroed_reps() {
    let zeroed = RepResult {
        rep: 1,
        raw_score: 1.0,
        score: 0.0,
        passed: false,
        category: FailureCategory::CanaryLeak,
        hits: vec![("canary".to_owned(), IntegrityFlag::CanaryLeak)],
        tokens: None,
    };
    let cases: Vec<CaseMetrics> = vec![metrics::case_metrics("a", 1.0, vec![zeroed])];
    let ablation = metrics::guard_ablation(&cases, "canary");
    assert!(ablation.with_guard.abs() < 1e-9);
    assert!((ablation.without_guard - 1.0).abs() < 1e-9);
    assert!((ablation.delta - 1.0).abs() < 1e-9);
}

#[test]
fn never_firing_guards_are_flagged_as_unmeasured_noise() {
    let cases = vec![metrics::case_metrics(
        "a",
        1.0,
        vec![rep(1.0, true, FailureCategory::Pass, None)],
    )];
    let coverage = metrics::trigger_coverage(&cases, &["canary", "turn_outcome"]);
    assert_eq!(coverage.len(), 2);
    assert!(coverage.iter().all(|c| c.noise && c.fires == 0));
}
