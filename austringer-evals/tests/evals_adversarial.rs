//! Adversarial channels (plan todo 44 QA): grader gaming cannot
//! inflate scores, unicode survives the whole pipeline, flaky
//! subjects are caught by the Pass^k floor (×3), stale dataset
//! parses are never served, and success claims assert SCORED RESULTS
//! from the real runner (never bare exit codes / `is_ok()`).

use austringer_evals::dataset::{CachedDataset, DatasetCache, EvalCase, EvalDataset};
use austringer_evals::grader::GraderSpec;
use austringer_evals::guard::{GuardSet, IntegrityFlag};
use austringer_evals::metrics::FailureCategory;
use austringer_evals::report::SweepReport;
use austringer_evals::runner::{ScriptedSubject, SweepPair, run_pair};

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

fn pr_lines(text: &str) -> Vec<String> {
    vec![
        serde_json::json!({
            "event": { "id": 1, "event": {
                "type": "tool_call", "call_id": "c1", "tool": "read_url",
                "arguments": { "url": "https://github.com/upstream/repo/pull/99" }
            }}
        })
        .to_string(),
        serde_json::json!({
            "event": { "id": 2, "event": {
                "type": "assistant",
                "blocks": [ { "type": "text", "text": text } ]
            }}
        })
        .to_string(),
        r#"{"type":"turn_result","ok":true,"outcome":"task_completed","turn":1}"#.to_owned(),
    ]
}

fn failed_turn_lines(text: &str) -> Vec<String> {
    vec![
        serde_json::json!({
            "event": { "id": 1, "event": {
                "type": "assistant",
                "blocks": [ { "type": "text", "text": text } ]
            }}
        })
        .to_string(),
        r#"{"type":"turn_result","ok":false,"outcome":"failed::InferenceExhausted","turn":1}"#
            .to_owned(),
    ]
}

fn dataset(cases: Vec<EvalCase>) -> EvalDataset {
    EvalDataset { cases }
}

fn guarded_case(id: &str, expected: &str, canary: Option<&str>) -> EvalCase {
    EvalCase {
        id: id.to_owned(),
        prompt: format!("solve {id}"),
        grader: GraderSpec::AllOf {
            expected: vec![expected.to_owned()],
        },
        threshold: 1.0,
        canary: canary.map(str::to_owned),
        tags: vec![],
    }
}

/// ADVERSARIAL (grader gaming): outputs that satisfy the grader but
/// leak the canary, open an upstream PR, or ride a FAILED turn are all
/// ZEROED — the untrusted output cannot inflate the score, and the
/// flags + fingerprints name the reason.
#[test]
fn untrusted_outputs_cannot_inflate_scores() {
    let ds = dataset(vec![
        guarded_case("leak", "42", Some("canary-abc-123")),
        guarded_case("pr-open", "42", None),
        guarded_case("failed-turn", "42", None),
        guarded_case("honest", "42", None),
    ]);
    let mut subject = ScriptedSubject::new();
    // Every output CONTAINS the expected string "42" — a grader-only
    // pipeline would score 1.0 on all four.
    subject.add("leak", vec![lines("canary-abc-123 and the answer is 42")]);
    subject.add("pr-open", vec![pr_lines("answer: 42")]);
    subject.add("failed-turn", vec![failed_turn_lines("42")]);
    subject.add("honest", vec![lines("the answer is 42")]);
    let guards = GuardSet::standard();
    let run = run_pair(
        &SweepPair {
            model: "gamer".to_owned(),
            profile: "p".to_owned(),
        },
        &ds,
        1,
        &mut subject,
        &guards,
    );
    let by_category = |id: &str| {
        run.cases
            .iter()
            .find(|c| c.case_id == id)
            .and_then(|c| c.reps.first())
            .map(|r| (r.score, r.category.name()))
            .unwrap_or((-1.0, "missing"))
    };
    assert_eq!(by_category("leak"), (0.0, "canary_leak"));
    assert_eq!(by_category("pr-open"), (0.0, "fair_internet_use"));
    assert_eq!(by_category("failed-turn"), (0.0, "turn_failed"));
    assert_eq!(by_category("honest"), (1.0, "pass"));
    // Raw scores prove the zeroing happened at the guard layer.
    let leak_rep = run
        .cases
        .iter()
        .find(|c| c.case_id == "leak")
        .and_then(|c| c.reps.first())
        .cloned()
        .expect("case present");
    assert!((leak_rep.raw_score - 1.0).abs() < 1e-9);
    assert!(
        leak_rep
            .hits
            .iter()
            .any(|(id, flag)| id == "canary" && *flag == IntegrityFlag::CanaryLeak)
    );
}

/// ADVERSARIAL (unicode): CJK prompts/expectations and emoji (incl. a
/// ZWJ sequence) score correctly through the whole pipeline.
#[test]
fn unicode_flows_through_the_whole_pipeline() {
    let ds = dataset(vec![
        guarded_case("cjk", "答え: 42", None),
        guarded_case("emoji", "thumbs \u{1f44d}\u{1f3fb} up", None),
    ]);
    let mut subject = ScriptedSubject::new();
    subject.add("cjk", vec![lines("計算結果 → 答え: 42")]);
    subject.add("emoji", vec![lines("done — thumbs \u{1f44d}\u{1f3fb} up!")]);
    let guards = GuardSet::standard();
    let run = run_pair(
        &SweepPair {
            model: "uni".to_owned(),
            profile: "p".to_owned(),
        },
        &ds,
        1,
        &mut subject,
        &guards,
    );
    assert!(
        run.cases
            .iter()
            .all(|c| c.pass_k && (c.mean_at_k - 1.0).abs() < 1e-9)
    );
}

/// ADVERSARIAL (flaky ×3): a subject that alternates pass/fail keeps a
/// decent Mean@k but collapses to Pass^k = 0 — the 24pt consistency
/// gap the floor exists to expose.
#[test]
fn flaky_subject_passes_mean_but_fails_the_pass_k_floor() {
    let ds = dataset(vec![guarded_case("flaky", "42", None)]);
    let mut subject = ScriptedSubject::new();
    subject.add("flaky", vec![lines("42"), lines("41"), lines("42")]);
    let guards = GuardSet::standard();
    let run = run_pair(
        &SweepPair {
            model: "flaky".to_owned(),
            profile: "p".to_owned(),
        },
        &ds,
        3,
        &mut subject,
        &guards,
    );
    let case = &run.cases[0];
    assert!(
        (case.mean_at_k - 2.0 / 3.0).abs() < 1e-9,
        "mean@k keeps the peak illusion"
    );
    assert!(!case.pass_k, "pass^k floor must catch the flake");
    assert_eq!(
        run.cases[0].reps[1].category,
        FailureCategory::GraderMismatch
    );
}

/// ADVERSARIAL (misleading_success): a subject whose runs ALL fail
/// still yields a "successful" sweep — the assertions must be on the
/// SCORED RESULTS (scores, categories, ranking), and they are.
#[test]
fn success_claims_assert_scored_results_not_runner_happiness() {
    let ds = dataset(vec![guarded_case("a", "42", None)]);
    let mut subject = ScriptedSubject::new();
    subject.add("a", vec![failed_turn_lines("42 — but the turn failed")]);
    let guards = GuardSet::standard();
    let run = run_pair(
        &SweepPair {
            model: "m".to_owned(),
            profile: "p".to_owned(),
        },
        &ds,
        1,
        &mut subject,
        &guards,
    );
    let report = SweepReport::from_runs(
        austringer_evals::sweep::Trigger::Scheduled,
        vec![run],
        &std::collections::BTreeMap::new(),
        &guards,
    );
    // The runner "succeeded" (a report exists); the SCORED result is
    // total failure — and that is what we assert.
    assert_eq!(report.pairs.len(), 1);
    assert!(report.pairs[0].metrics.pass_k.abs() < 1e-9);
    assert_eq!(
        report.pairs[0].metrics.fingerprint.get("turn_failed"),
        Some(&1),
        "fingerprint must name the failure"
    );
    let ranked = &report.ranking;
    assert_eq!(ranked.len(), 1);
    assert!(ranked[0].pass_k.abs() < 1e-9);
}

/// ADVERSARIAL (stale_state): rewriting the dataset file under the
/// cache forces a Reloaded result — the stale parse is never served.
#[test]
fn changed_dataset_files_reload_never_serve_stale_parses() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let path = dir.path().join("ds.json");
    let v1 = serde_json::json!({
        "cases": [{ "id": "v1-only", "prompt": "p",
            "grader": { "kind": "all_of", "expected": ["x"] } }]
    })
    .to_string();
    std::fs::write(&path, &v1).unwrap_or_default();
    let mut cache = DatasetCache::new();
    match cache.get(&path).unwrap_or_else(|e| panic!("{e}")) {
        CachedDataset::Fresh(d) => assert_eq!(d.cases[0].id, "v1-only"),
        CachedDataset::Reloaded(_) => panic!("first load must be fresh"),
    }
    let v2 = serde_json::json!({
        "cases": [{ "id": "v2-only", "prompt": "p",
            "grader": { "kind": "all_of", "expected": ["y"] } }]
    })
    .to_string();
    std::fs::write(&path, &v2).unwrap_or_default();
    match cache.get(&path).unwrap_or_else(|e| panic!("{e}")) {
        CachedDataset::Reloaded(d) => {
            assert_eq!(d.cases[0].id, "v2-only", "the new parse must be served");
        }
        CachedDataset::Fresh(d) => {
            panic!("stale parse served: {}", d.cases[0].id);
        }
    }
}
