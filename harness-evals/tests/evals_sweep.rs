//! The sweep::execute orchestration path (the nightly-sweep entry):
//! a validated ≥2×2 experiment grid driven through registered
//! subjects assembles a full report; a pair without a registered
//! subject is a typed error, never a silent skip.

use std::collections::BTreeMap;

use harness_evals::dataset::{EvalCase, EvalDataset};
use harness_evals::grader::GraderSpec;
use harness_evals::guard::GuardSet;
use harness_evals::runner::{ScriptedSubject, Subject, SweepPair};
use harness_evals::sweep::{Subjects, SweepConfig, SweepConfigError, SweepMode, Trigger, execute};

fn grid_dataset() -> EvalDataset {
    EvalDataset {
        cases: vec![EvalCase {
            id: "sum".to_owned(),
            prompt: "sum".to_owned(),
            grader: GraderSpec::AllOf {
                expected: vec!["4".to_owned()],
            },
            threshold: 1.0,
            canary: None,
            tags: vec![],
        }],
    }
}

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

#[test]
fn execute_runs_the_grid_and_assembles_the_report() {
    let dataset = grid_dataset();
    let guards = GuardSet::standard();
    let mut strong = ScriptedSubject::new();
    strong.add("sum", vec![ok_lines("4")]);
    let mut weak = ScriptedSubject::new();
    weak.add("sum", vec![ok_lines("5")]);

    let mut subjects: BTreeMap<String, ScriptedSubject> = BTreeMap::new();
    subjects.insert("alpha".to_owned(), strong);
    subjects.insert("beta".to_owned(), weak);
    let mut subjects: Subjects<'_> = subjects
        .iter_mut()
        .map(|(model, subject)| (model.clone(), subject as &mut dyn Subject))
        .collect();

    let config = SweepConfig {
        trigger: Trigger::Scheduled,
        mode: SweepMode::Experiment,
        pairs: vec![
            SweepPair {
                model: "alpha".to_owned(),
                profile: "fast".to_owned(),
            },
            SweepPair {
                model: "alpha".to_owned(),
                profile: "thorough".to_owned(),
            },
            SweepPair {
                model: "beta".to_owned(),
                profile: "fast".to_owned(),
            },
            SweepPair {
                model: "beta".to_owned(),
                profile: "thorough".to_owned(),
            },
        ],
        reps: 2,
    };
    let report = execute(&config, &dataset, &mut subjects, &BTreeMap::new(), &guards)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report.trigger, Trigger::Scheduled);
    assert_eq!(report.pairs.len(), 4);
    // alpha solves the case on BOTH profiles; beta on neither — and
    // the ranked table leads with alpha (scored results, not hopes).
    assert_eq!(report.ranking.len(), 4);
    assert_eq!(report.ranking[0].pair.model, "alpha");
    assert!((report.ranking[0].pass_k - 1.0).abs() < 1e-9);
    for row in report.ranking.iter().take(2) {
        assert_eq!(row.pair.model, "alpha");
    }
    for row in report.ranking.iter().skip(2) {
        assert_eq!(row.pair.model, "beta");
        assert!(row.pass_k.abs() < 1e-9);
    }
    // Cards default (no card registered) ⇒ unmatched ⇒ not publishable.
    assert!(report.pairs.iter().all(|p| !p.card.publishable()));
}

#[test]
fn missing_subjects_are_typed_errors_never_silent_skips() {
    let dataset = grid_dataset();
    let guards = GuardSet::standard();
    let mut only_alpha: BTreeMap<String, ScriptedSubject> = BTreeMap::new();
    only_alpha.insert("alpha".to_owned(), {
        let mut s = ScriptedSubject::new();
        s.add("sum", vec![ok_lines("4")]);
        s
    });
    let mut subjects: Subjects<'_> = only_alpha
        .iter_mut()
        .map(|(model, subject)| (model.clone(), subject as &mut dyn Subject))
        .collect();
    let config = SweepConfig {
        trigger: Trigger::Manual,
        mode: SweepMode::Experiment,
        pairs: vec![
            SweepPair {
                model: "alpha".to_owned(),
                profile: "p1".to_owned(),
            },
            SweepPair {
                model: "ghost".to_owned(),
                profile: "p2".to_owned(),
            },
            SweepPair {
                model: "alpha".to_owned(),
                profile: "p3".to_owned(),
            },
            SweepPair {
                model: "ghost".to_owned(),
                profile: "p4".to_owned(),
            },
        ],
        reps: 1,
    };
    match execute(&config, &dataset, &mut subjects, &BTreeMap::new(), &guards) {
        Err(SweepConfigError::MissingSubject { model }) => assert_eq!(model, "ghost"),
        other => panic!("expected MissingSubject, got {other:?}"),
    }
}
