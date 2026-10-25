//! Unit tests for the runner surface (scripted subject rep cycling,
//! deterministic ranking) — moved out of `src/runner.rs` to keep the
//! source file under the 250-pure-LOC ceiling.

use austringer_evals::dataset::{EvalCase, EvalDataset};
use austringer_evals::grader::GraderSpec;
use austringer_evals::metrics::{CaseMetrics, FailureCategory, RepResult, case_metrics};
use austringer_evals::runner::{PairRun, ScriptedSubject, Subject, SweepPair, rank};

fn ok_lines(text: &str) -> Vec<String> {
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

fn dataset_two_cases() -> EvalDataset {
    EvalDataset {
        cases: vec![
            EvalCase {
                id: "a".to_owned(),
                prompt: "pa".to_owned(),
                grader: GraderSpec::AllOf {
                    expected: vec!["42".to_owned()],
                },
                threshold: 1.0,
                canary: None,
                tags: vec![],
            },
            EvalCase {
                id: "b".to_owned(),
                prompt: "pb".to_owned(),
                grader: GraderSpec::AllOf {
                    expected: vec!["13".to_owned()],
                },
                threshold: 1.0,
                canary: None,
                tags: vec![],
            },
        ],
    }
}

#[test]
fn scripted_subject_cycles_reps_so_flakiness_is_modelable() {
    let mut subject = ScriptedSubject::new();
    subject.add("a", vec![ok_lines("42"), ok_lines("wrong")]);
    let dataset = dataset_two_cases();
    let case_a = &dataset.cases[0];
    assert!(subject.run(case_a, 1).is_ok());
    assert!(subject.run(case_a, 2).is_ok());
    assert!(subject.run(case_a, 3).is_ok());
    let missing = subject.run(&dataset.cases[1], 1);
    assert!(missing.is_err());
}

#[test]
fn rank_orders_by_pass_k_then_mean_then_id_and_is_total() {
    let passed_case = |id: &str, score: f64| {
        let rep = RepResult {
            rep: 1,
            raw_score: score,
            score,
            passed: score >= 1.0,
            category: if score >= 1.0 {
                FailureCategory::Pass
            } else {
                FailureCategory::GraderMismatch
            },
            hits: Vec::new(),
            tokens: Some(10),
        };
        case_metrics(id, 1.0, vec![rep])
    };
    let run = |model: &str, cases: Vec<CaseMetrics>| PairRun {
        pair: SweepPair {
            model: model.to_owned(),
            profile: "p".to_owned(),
        },
        cases,
    };
    let runs = vec![
        run("zeta", vec![passed_case("a", 0.0)]),
        run("beta", vec![passed_case("a", 1.0)]),
        run("alpha", vec![passed_case("a", 1.0)]),
    ];
    let ranked = rank(&runs);
    let labels: Vec<String> = ranked.iter().map(|r| r.pair.model.clone()).collect();
    assert_eq!(labels, vec!["alpha", "beta", "zeta"]);
    assert!((ranked[0].pass_k - 1.0).abs() < 1e-9);
    assert!(ranked[2].pass_k.abs() < 1e-9);
}
