//! Full-stack happy + failure channels (plan todo 44 QA): a dataset
//! runs through scripted fake inference via the REAL runner, graders
//! score deterministically, the ranking table and report artifact are
//! produced, and the failure channels (malformed dataset, empty
//! dataset, grader error non-fatal per case) are typed.

use std::collections::BTreeMap;

use austringer_evals::dataset::{DatasetCache, DatasetError, EvalDataset};
use austringer_evals::grader::GraderSpec;
use austringer_evals::guard::GuardSet;
use austringer_evals::pinning::parse_expected_failures;
use austringer_evals::report::{AustringerCard, SourceFingerprint};
use austringer_evals::runner::{ScriptedSubject, Subject, SweepPair, rank, run_pair};
use austringer_evals::sweep::{SweepConfig, SweepMode, Trigger, validate};

fn case_json(id: &str, expected: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "prompt": format!("solve {id}"),
        "grader": { "kind": "all_of", "expected": [expected] }
    })
}

fn dataset_doc() -> String {
    serde_json::json!({
        "cases": [case_json("math-1", "42"), case_json("math-2", "17")]
    })
    .to_string()
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

/// HAPPY: dataset → scripted inference → deterministic graders →
/// ranking table → report artifact on disk. Asserts SCORED RESULTS
/// (scores, ordering, artifact bytes) — never bare `is_ok()`.
#[test]
fn full_sweep_scores_ranks_and_writes_the_report_artifact() {
    let dataset = EvalDataset::parse(&dataset_doc()).unwrap_or_else(|e| panic!("{e}"));
    let guards = GuardSet::standard();

    let mut strong = ScriptedSubject::new();
    strong.add("math-1", vec![ok_lines("the answer is 42")]);
    strong.add("math-2", vec![ok_lines("17 it is")]);
    let mut weak = ScriptedSubject::new();
    weak.add("math-1", vec![ok_lines("the answer is 41")]);
    weak.add("math-2", vec![ok_lines("18 it is")]);

    let strong_run = run_pair(
        &SweepPair {
            model: "mini".to_owned(),
            profile: "baseline".to_owned(),
        },
        &dataset,
        3,
        &mut strong,
        &guards,
    );
    let weak_run = run_pair(
        &SweepPair {
            model: "nano".to_owned(),
            profile: "baseline".to_owned(),
        },
        &dataset,
        3,
        &mut weak,
        &guards,
    );

    // Scored results from the real runner: strong solves both at
    // score 1.0; weak solves neither.
    assert_eq!(strong_run.cases.len(), 2);
    for case in &strong_run.cases {
        assert!(case.pass_k, "case {} must pass all reps", case.case_id);
        assert!((case.mean_at_k - 1.0).abs() < 1e-9);
    }
    for case in &weak_run.cases {
        assert!(!case.pass_k);
        assert!(case.mean_at_k.abs() < 1e-9);
    }

    // Deterministic ranking: mini first.
    let ranked = rank(&[weak_run.clone(), strong_run.clone()]);
    assert_eq!(ranked[0].pair.model, "mini");
    assert_eq!(ranked[1].pair.model, "nano");

    // Report + artifact + pins (acceptance: nightly sweep produces a
    // report artifact; known failure pinned and annotated).
    let mut cards = BTreeMap::new();
    let mut card = AustringerCard {
        model: "mini".to_owned(),
        profile: "baseline".to_owned(),
        settings: BTreeMap::from([("effort".to_owned(), "default".to_owned())]),
        source: SourceFingerprint {
            commit: Some("238ad5e".to_owned()),
            dirty: false,
            dirty_files: vec![],
        },
        baseline_ref: Some("official-v1".to_owned()),
    };
    cards.insert("mini/baseline".to_owned(), card.clone());
    card.model = "nano".to_owned();
    cards.insert("nano/baseline".to_owned(), card);

    let config = SweepConfig {
        trigger: Trigger::Scheduled,
        mode: SweepMode::Regression {
            locked_profile: "baseline".to_owned(),
        },
        pairs: vec![
            SweepPair {
                model: "mini".to_owned(),
                profile: "baseline".to_owned(),
            },
            SweepPair {
                model: "nano".to_owned(),
                profile: "baseline".to_owned(),
            },
        ],
        reps: 3,
    };
    assert_eq!(validate(&config), Ok(()));

    let mut report = austringer_evals::report::SweepReport::from_runs(
        Trigger::Scheduled,
        vec![strong_run, weak_run],
        &cards,
        &guards,
    );
    let pins = parse_expected_failures(
        "version: 1\nfailures:\n  - case: math-2\n    model: nano\n    reason: known off-by-one\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    report.apply_pins(&pins);
    // nano fails BOTH cases; math-2 is pinned (annotated), math-1 is
    // an unpinned regression that must surface.
    assert_eq!(report.pins.pinned_confirmed.len(), 1);
    assert_eq!(report.pins.pinned_confirmed[0].case, "math-2");
    assert_eq!(report.pins.unpinned.len(), 1);
    assert_eq!(report.pins.unpinned[0].case, "math-1");

    let table = report.render_table();
    assert!(table.contains("mini"), "{table}");
    assert!(table.contains("pass^k"), "{table}");

    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let artifact = dir.path().join("sweep.json");
    report
        .write_artifact(&artifact)
        .unwrap_or_else(|e| panic!("{e}"));
    let bytes = std::fs::read_to_string(&artifact).unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&bytes).unwrap_or_default();
    assert_eq!(parsed["trigger"], serde_json::json!("scheduled"));
    assert_eq!(parsed["pairs"].as_array().map(Vec::len), Some(2));
}

/// FAILURE: malformed and empty datasets are typed errors.
#[test]
fn malformed_and_empty_datasets_are_typed_errors() {
    let malformed = EvalDataset::parse("{ definitely not json");
    assert!(matches!(malformed, Err(DatasetError::Malformed { .. })));
    let empty = EvalDataset::parse(r#"{"cases":[]}"#);
    assert_eq!(empty.err(), Some(DatasetError::Empty));
}

/// FAILURE: a grader error is typed and NON-FATAL per case — the
/// sweep continues and the case is fingerprinted `grader_error`.
/// (EvalDataset is a plain struct, so re-scored/foreign datasets can
/// carry unvalidated specs — exactly the seam where grading can fail.)
#[test]
fn grader_errors_are_typed_and_non_fatal_per_case() {
    use austringer_evals::dataset::EvalCase;

    let dataset = EvalDataset {
        cases: vec![
            EvalCase {
                id: "broken-grader".to_owned(),
                prompt: "p1".to_owned(),
                grader: GraderSpec::AllOf { expected: vec![] }, // invalid at score time
                threshold: 1.0,
                canary: None,
                tags: vec![],
            },
            EvalCase {
                id: "fine".to_owned(),
                prompt: "p2".to_owned(),
                grader: GraderSpec::AllOf {
                    expected: vec!["ok".to_owned()],
                },
                threshold: 1.0,
                canary: None,
                tags: vec![],
            },
        ],
    };
    let mut subject = ScriptedSubject::new();
    subject.add("broken-grader", vec![ok_lines("anything")]);
    subject.add("fine", vec![ok_lines("ok")]);
    let guards = GuardSet::standard();
    let run = run_pair(
        &SweepPair {
            model: "m".to_owned(),
            profile: "p".to_owned(),
        },
        &dataset,
        1,
        &mut subject,
        &guards,
    );
    let broken = run
        .cases
        .iter()
        .find(|c| c.case_id == "broken-grader")
        .cloned()
        .expect("case present");
    let fine = run
        .cases
        .iter()
        .find(|c| c.case_id == "fine")
        .cloned()
        .expect("case present");
    assert_eq!(
        broken.reps[0].category.name(),
        "grader_error",
        "the grader-error rep must be fingerprinted, not swallowed"
    );
    assert!(!broken.pass_k);
    assert!(fine.pass_k, "the sweep must continue past the broken case");
}

/// The dataset cache exists and round-trips (the stale-reload behavior
/// is adversarially pinned in evals_adversarial.rs).
#[test]
fn dataset_cache_serves_parsed_datasets() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let path = dir.path().join("ds.json");
    std::fs::write(&path, dataset_doc()).unwrap_or_default();
    let mut cache = DatasetCache::new();
    let first = cache.get(&path).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(first.dataset().cases.len(), 2);
}

/// Subject contract: `dyn Subject` is object-safe and the runner
/// drives it per (case, rep).
#[test]
fn subject_trait_is_object_safe_and_driven_per_case_and_rep() {
    struct CountingSubject {
        calls: u32,
    }
    impl Subject for CountingSubject {
        fn run(
            &mut self,
            case: &austringer_evals::dataset::EvalCase,
            rep: u32,
        ) -> Result<Vec<String>, austringer_evals::runner::SubjectError> {
            self.calls += 1;
            Ok(ok_lines(&format!("{}-{}", case.id, rep)))
        }
    }
    let dataset = EvalDataset::parse(&dataset_doc()).unwrap_or_else(|e| panic!("{e}"));
    let mut counting: Box<dyn Subject> = Box::new(CountingSubject { calls: 0 });
    let guards = GuardSet::standard();
    let run = run_pair(
        &SweepPair {
            model: "m".to_owned(),
            profile: "p".to_owned(),
        },
        &dataset,
        2,
        counting.as_mut(),
        &guards,
    );
    assert_eq!(run.cases.len(), 2);
    assert_eq!(run.cases[0].reps.len(), 2);
    assert!(
        run.cases[0]
            .reps
            .iter()
            .all(|r| r.category.name() == "grader_mismatch")
    );
}
