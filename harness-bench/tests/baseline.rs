//! Committed baselines for the >10% regression gate. Machine-independent
//! deterministic metrics only (allocation counts, render counts, byte
//! sizes); staleness against the live scenario set is a typed error.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use harness_bench::baseline::{Baseline, BaselineError};

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

#[test]
fn baseline_round_trips_through_json() {
    let mut metrics = BTreeMap::new();
    metrics.insert("tui_frame.alloc_count".to_owned(), 4321.0);
    let baseline = Baseline {
        schema: 1,
        recorded_on: Some("windows".to_owned()),
        metrics,
    };
    let json = baseline.to_json_pretty().expect("serialize");
    let back = Baseline::from_json_str(&json).expect("parse");
    assert_eq!(back, baseline);
}

#[test]
fn malformed_baseline_is_typed() {
    let err = Baseline::from_json_str("{ nope").expect_err("malformed");
    assert!(matches!(err, BaselineError::Malformed(_)), "got: {err}");
}

#[test]
fn wrong_schema_version_is_typed_and_names_both_versions() {
    let json = r#"{"schema": 2, "recorded_on": null, "metrics": {}}"#;
    let err = Baseline::from_json_str(json).expect_err("schema mismatch");
    assert!(matches!(
        err,
        BaselineError::Schema {
            found: 2,
            expected: 1
        }
    ));
}

#[test]
fn fresh_baseline_passes_freshness_check() {
    let baseline = Baseline {
        schema: 1,
        recorded_on: None,
        metrics: BTreeMap::from([("a".to_owned(), 1.0), ("b".to_owned(), 2.0)]),
    };
    baseline
        .check_freshness(&set(&["a", "b"]))
        .expect("fresh baseline is ok");
}

#[test]
fn stale_baseline_lists_missing_and_unknown_metrics() {
    let baseline = Baseline {
        schema: 1,
        recorded_on: None,
        metrics: BTreeMap::from([("a".to_owned(), 1.0), ("c".to_owned(), 3.0)]),
    };
    let err = baseline
        .check_freshness(&set(&["a", "b"]))
        .expect_err("stale");
    match err {
        BaselineError::Stale { missing, unknown } => {
            assert_eq!(missing, vec!["b".to_owned()]);
            assert_eq!(unknown, vec!["c".to_owned()]);
        }
        other => panic!("expected Stale, got {other}"),
    }
}

#[test]
fn empty_expected_set_against_empty_baseline_is_fresh() {
    let baseline = Baseline {
        schema: 1,
        recorded_on: None,
        metrics: BTreeMap::new(),
    };
    baseline
        .check_freshness(&BTreeSet::new())
        .expect("empty vs empty is fresh");
}

#[test]
fn scoped_check_tolerates_baseline_entries_for_skipped_scenarios_only() {
    // A run with an allowed skip produces fewer metrics; the baseline may
    // keep the skipped scenario's entry (it belongs to the registry), but
    // every PRODUCED metric must be present, and baseline keys outside the
    // registry's known names are stale.
    let baseline = Baseline {
        schema: 1,
        recorded_on: None,
        metrics: BTreeMap::from([
            ("repomap.tokens".to_owned(), 900.0),
            ("sessions.page_bytes".to_owned(), 210_000.0), // skipped this run
        ]),
    };
    let registry = set(&["repomap.tokens", "sessions.page_bytes"]);
    let produced = set(&["repomap.tokens"]);
    baseline
        .check_scope(&registry, &produced)
        .expect("scoped freshness ok");

    let mut missing = baseline.clone();
    missing.metrics.remove("repomap.tokens");
    let err = missing
        .check_scope(&registry, &produced)
        .expect_err("produced metric missing");
    assert!(matches!(err, BaselineError::Stale { .. }), "{err}");

    let mut alien = baseline;
    alien.metrics.insert("ghost.metric".to_owned(), 1.0);
    let err = alien
        .check_scope(&registry, &produced)
        .expect_err("baseline key outside registry");
    assert!(matches!(err, BaselineError::Stale { .. }), "{err}");
}
