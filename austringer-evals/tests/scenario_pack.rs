//! Scenario-pack + driver integration tests (todo 26). The committed
//! hermetic pack is the receipt that the plan's headline mechanisms
//! (ask-first fallback, task(category) fan-out, plan-gate refusal,
//! run_tests report shape, search_sessions scoping) are exercised by
//! the conformance sweep — these tests pin that the pack loads, every
//! pinned case conforms, corrupted fixtures fail LOUDLY with the
//! cassette named, and the driver produces an austringer-matched report.

mod pack {
    use austringer_evals::conformance::ConformanceVerdict;
    use austringer_evals::runner::Subject;
    use austringer_evals::scenarios::{self, PINNED_CASES, ScenarioError};
    use std::path::{Path, PathBuf};

    /// Compile-time-anchored pack dir (nextest archive mode and
    /// llvm-cov run test binaries from a different CWD, and a
    /// hardcoded `"\\scenarios"` suffix breaks on unix, where the
    /// backslash is not a path separator).
    fn pack_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios")
    }

    fn copy_pack(dest: &Path) -> PathBuf {
        let src = &pack_dir();
        let out = dest.join("scenarios");
        std::fs::create_dir_all(out.join("cassettes")).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(entry.path(), out.join(entry.file_name())).unwrap();
            }
        }
        for entry in std::fs::read_dir(src.join("cassettes")).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), out.join("cassettes").join(entry.file_name())).unwrap();
        }
        out
    }

    /// RECEIPT: the committed pack covers exactly the five plan
    /// mechanisms, in dataset order, each with a cassette script, and
    /// pins exactly the two golden-prefix cases.
    #[test]
    fn committed_pack_covers_the_five_mechanisms_with_cassettes() {
        let pack = scenarios::load(&pack_dir()).unwrap_or_else(|e| panic!("{e}"));
        let ids: Vec<&str> = pack.dataset.cases.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "ask-first-fallback",
                "task-category-fan-out",
                "plan-gate-refusal",
                "run-tests-report-shape",
                "search-sessions-scoping",
            ]
        );
        assert_eq!(pack.pinned_case_ids(), PINNED_CASES.to_vec());
        let mut subject = pack.build_subject();
        for case in &pack.dataset.cases {
            let lines = subject.run(case, 1).unwrap_or_else(|e| panic!("{e}"));
            assert!(!lines.is_empty(), "empty script for {}", case.id);
        }
    }

    /// RECEIPT (conformance): every pinned case's cassette conforms to
    /// its committed golden prefix.
    #[test]
    fn every_pinned_case_conforms_to_its_golden_prefix() {
        let pack = scenarios::load(&pack_dir()).unwrap_or_else(|e| panic!("{e}"));
        let findings = pack.conformance_findings();
        assert!(
            findings.is_empty(),
            "golden-prefix drift in the committed pack: {findings:?}"
        );
    }

    /// FAILURE PATH: a corrupted cassette fails the pack load LOUDLY,
    /// with the offending cassette file named (never a bare parse
    /// panic, never a silent skip).
    #[test]
    fn corrupted_cassette_fails_loudly_naming_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let pack_dir = copy_pack(tmp.path());
        let target = pack_dir.join("cassettes").join("plan-gate-refusal.ndjson");
        std::fs::write(&target, "{corrupted not json\n").unwrap();
        let err = scenarios::load(&pack_dir)
            .err()
            .unwrap_or_else(|| panic!("corrupted cassette must fail the load"));
        let msg = err.to_string();
        assert!(
            msg.contains("plan-gate-refusal.ndjson"),
            "cassette not named in error: {msg}"
        );
        assert!(matches!(err, ScenarioError::Cassette { .. }));
    }

    /// FAILURE PATH (conformance drift): rewriting the first fan-out
    /// tool call in a copy of the pinned cassette surfaces a NAMED
    /// drift with expected vs actual prefixes — the real fixture is
    /// never touched.
    #[test]
    fn prefix_drift_in_a_pinned_cassette_is_a_named_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let pack_dir = copy_pack(tmp.path());
        let target = pack_dir
            .join("cassettes")
            .join("task-category-fan-out.ndjson");
        let body = std::fs::read_to_string(&target).unwrap();
        std::fs::write(
            &target,
            body.replacen(r#""tool":"task""#, r#""tool":"task_v2""#, 1),
        )
        .unwrap();
        let pack = scenarios::load(&pack_dir).unwrap_or_else(|e| panic!("{e}"));
        let findings = pack.conformance_findings();
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].case_id, "task-category-fan-out");
        assert!(
            findings[0]
                .cassette
                .contains("task-category-fan-out.ndjson"),
            "cassette not named: {}",
            findings[0].cassette
        );
        assert_eq!(
            findings[0].verdict,
            ConformanceVerdict::Drift {
                expected: vec!["task".to_owned(), "task".to_owned()],
                actual: vec!["task_v2".to_owned(), "task".to_owned()],
            }
        );
    }

    /// FAILURE PATH (pack integrity): a pin set that is not exactly
    /// the pinned mechanism cases (dropped or unknown) fails closed.
    #[test]
    fn pin_sets_other_than_the_pinned_mechanisms_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let pack_dir = copy_pack(tmp.path());
        let pins_path = pack_dir.join("conformance-pins.json");
        let mut pins: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&pins_path).unwrap()).unwrap();
        pins.as_object_mut()
            .unwrap()
            .remove("task-category-fan-out")
            .unwrap();
        std::fs::write(&pins_path, serde_json::to_string(&pins).unwrap()).unwrap();
        let err = scenarios::load(&pack_dir)
            .err()
            .unwrap_or_else(|| panic!("dropped pin must fail the load"));
        assert!(matches!(err, ScenarioError::PinSetMismatch { .. }));
    }
}

mod driver {
    use austringer_evals::driver::{self, DriverArgs, DriverError};
    use austringer_evals::report::SourceFingerprint;
    use austringer_evals::sweep::Trigger;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    /// Compile-time-anchored pack dir (nextest archive mode and
    /// llvm-cov run test binaries from a different CWD, and a
    /// hardcoded `"\\scenarios"` suffix breaks on unix, where the
    /// backslash is not a path separator).
    fn pack_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios")
    }

    fn copy_pack(dest: &Path) -> PathBuf {
        let src = &pack_dir();
        let out = dest.join("scenarios");
        std::fs::create_dir_all(out.join("cassettes")).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(entry.path(), out.join(entry.file_name())).unwrap();
            }
        }
        for entry in std::fs::read_dir(src.join("cassettes")).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), out.join("cassettes").join(entry.file_name())).unwrap();
        }
        out
    }

    fn source() -> SourceFingerprint {
        SourceFingerprint {
            commit: Some("abc123def456".to_owned()),
            dirty: false,
            dirty_files: Vec::new(),
        }
    }

    /// HAPPY PATH: the hermetic conformance sweep over the committed
    /// pack exits clean and writes an austringer-MATCHED report (matched
    /// baseline, pass^k floor, no DO-NOT-PUBLISH row).
    #[test]
    fn happy_sweep_writes_matched_artifacts_and_exits_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let args = DriverArgs {
            trigger: Trigger::Manual,
            fixtures: pack_dir(),
            report: tmp.path().join("report.json"),
        };
        let verdict = driver::run(&args, &source()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(verdict.exit_code(), 0);
        assert!(verdict.table.contains("baseline=hermetic-scenario-pack-v1"));
        assert!(!verdict.table.contains("DO NOT PUBLISH"));
        assert!((verdict.report.ranking[0].pass_k - 1.0).abs() < 1e-9);
        let json = std::fs::read_to_string(&args.report).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert!(parsed.get("pairs").is_some());
        let table = std::fs::read_to_string(tmp.path().join("report.table.txt")).unwrap();
        assert!(table.contains("pass^k"));
    }

    /// FAILURE PATH: running the driver against a corrupted pack is a
    /// config error (exit 3) naming the cassette — loud, typed, non-zero.
    #[test]
    fn corrupted_pack_is_a_config_error_naming_the_cassette() {
        let tmp = tempfile::tempdir().unwrap();
        let pack_dir = copy_pack(tmp.path());
        std::fs::write(
            pack_dir.join("cassettes").join("ask-first-fallback.ndjson"),
            "not ndjson at all\n",
        )
        .unwrap();
        let args = DriverArgs {
            trigger: Trigger::Scheduled,
            fixtures: pack_dir,
            report: tmp.path().join("report.json"),
        };
        let err = driver::run(&args, &source())
            .err()
            .unwrap_or_else(|| panic!("corrupted pack must fail the driver"));
        assert!(matches!(err, DriverError::Pack(_)));
        assert_eq!(err.exit_code(), 3);
        assert!(
            err.to_string().contains("ask-first-fallback.ndjson"),
            "cassette not named: {err}"
        );
    }

    /// PINNED (policy): the trigger vocabulary the driver accepts has
    /// no PR-gate variant — scheduled and manual only.
    #[test]
    fn trigger_argument_parses_the_two_policy_variants_only() {
        assert_eq!(driver::parse_trigger("scheduled"), Ok(Trigger::Scheduled));
        assert_eq!(driver::parse_trigger("manual"), Ok(Trigger::Manual));
        for rejected in ["pr", "pull_request", "release", ""] {
            assert!(
                driver::parse_trigger(rejected).is_err(),
                "{rejected:?} must not parse as a trigger"
            );
        }
    }

    /// The Austringer card carries the pack's matched baseline and the
    /// probed source fingerprint verbatim (dirty trees stay visible).
    #[test]
    fn austringer_card_carries_baseline_and_source_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        let args = DriverArgs {
            trigger: Trigger::Scheduled,
            fixtures: pack_dir(),
            report: tmp.path().join("report.json"),
        };
        let dirty = SourceFingerprint {
            commit: None,
            dirty: true,
            dirty_files: vec!["scenarios/dataset.json".to_owned()],
        };
        let verdict = driver::run(&args, &dirty).unwrap_or_else(|e| panic!("{e}"));
        assert!(verdict.table.contains("dirty:1 files"));
        assert_eq!(verdict.report.pairs.len(), 1);
        assert_eq!(
            verdict.report.pairs[0].card.settings,
            BTreeMap::from([("subject".to_owned(), "scripted-cassette".to_owned())])
        );
    }
}
