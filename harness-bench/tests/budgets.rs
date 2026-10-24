//! Perf budgets of record (todo 43): absolute wall/RSS/size ceilings plus the
//! regression tolerance. Parsing is strict — malformed config fails typed,
//! never silently falls back to defaults.

use std::path::Path;

use harness_bench::budgets::{BudgetConfig, BudgetConfigError};

#[test]
fn plan_defaults_match_todo_43_budgets() {
    let cfg = BudgetConfig::plan_defaults();
    assert_eq!(cfg.cold_start_ms, 50, "cold start budget <50ms");
    assert_eq!(
        cfg.idle_max_renders, 1,
        "100k no-op events -> at most 1 render"
    );
    assert_eq!(cfg.rss_mb, 80, "RSS budget <80MB (todo 43 figure)");
    assert_eq!(cfg.p99_frame_ms, 16, "p99 render budget <16ms");
    assert_eq!(cfg.binary_mb, 40, "binary-size budget <40MB");
    assert_eq!(
        cfg.regression_tolerance_pct, 10,
        ">10% regression fails the gate"
    );
    assert_eq!(cfg.frame_budget_ms, 10, "Zed-style 10ms frame budget");
    assert_eq!(cfg.hang_threshold_ms, 100, "hang threshold 100ms");
}

#[test]
fn from_json_round_trips_custom_budgets() {
    let json = r#"{
        "cold_start_ms": 60,
        "idle_max_renders": 2,
        "rss_mb": 90,
        "p99_frame_ms": 20,
        "binary_mb": 35,
        "regression_tolerance_pct": 5,
        "frame_budget_ms": 8,
        "hang_threshold_ms": 120
    }"#;
    let cfg = BudgetConfig::from_json_str(json).expect("valid budgets parse");
    assert_eq!(cfg.cold_start_ms, 60);
    assert_eq!(cfg.regression_tolerance_pct, 5);
    let round =
        BudgetConfig::from_json_str(&cfg.to_json().expect("serialize")).expect("round-trip parses");
    assert_eq!(round, cfg);
}

#[test]
fn malformed_json_is_a_typed_error() {
    let err = BudgetConfig::from_json_str("{ not json").expect_err("malformed JSON");
    assert!(matches!(err, BudgetConfigError::Malformed(_)), "got: {err}");
}

#[test]
fn unknown_field_is_rejected_strictly() {
    let json = r#"{"cold_start_ms": 50, "surprise": 1}"#;
    let err = BudgetConfig::from_json_str(json).expect_err("unknown field");
    assert!(matches!(err, BudgetConfigError::Malformed(_)), "got: {err}");
}

#[test]
fn zero_or_negative_budget_is_out_of_range() {
    // Complete config (strict schema) with the offending value: validation
    // must reject it typed, not silently clamp or default.
    let zero = r#"{
        "cold_start_ms": 0,
        "idle_max_renders": 1,
        "rss_mb": 80,
        "p99_frame_ms": 16,
        "binary_mb": 40,
        "regression_tolerance_pct": 10,
        "frame_budget_ms": 10,
        "hang_threshold_ms": 100
    }"#;
    let err = BudgetConfig::from_json_str(zero).expect_err("cold_start_ms=0");
    assert!(matches!(
        err,
        BudgetConfigError::OutOfRange { field, value: 0, .. } if field == "cold_start_ms"
    ));
}

#[test]
fn regression_tolerance_above_100_is_out_of_range() {
    let json = r#"{
        "cold_start_ms": 50,
        "idle_max_renders": 1,
        "rss_mb": 80,
        "p99_frame_ms": 16,
        "binary_mb": 40,
        "regression_tolerance_pct": 101,
        "frame_budget_ms": 10,
        "hang_threshold_ms": 100
    }"#;
    let err = BudgetConfig::from_json_str(json).expect_err("tolerance=101");
    assert!(matches!(
        err,
        BudgetConfigError::OutOfRange { field, value: 101, .. } if field == "regression_tolerance_pct"
    ));
}

#[test]
fn missing_field_is_a_typed_malformed_error_not_a_default() {
    let json = r#"{"cold_start_ms": 50}"#;
    let err = BudgetConfig::from_json_str(json).expect_err("missing fields must not default");
    assert!(matches!(err, BudgetConfigError::Malformed(_)), "got: {err}");
}

#[test]
fn media_format_overrides_parse_and_round_trip() {
    // Given: the committed budgets.json shape - avif carries its own
    // encode timeout (plan M28: rav1e is tens-of-seconds slow by design
    // and is excluded from the <=1s pure round-trip line).
    let json = r#"{
        "cold_start_ms": 50,
        "idle_max_renders": 1,
        "rss_mb": 80,
        "p99_frame_ms": 16,
        "binary_mb": 40,
        "regression_tolerance_pct": 10,
        "frame_budget_ms": 10,
        "hang_threshold_ms": 100,
        "media_format_overrides": {
            "avif": {"encode_timeout_ms": 30000}
        }
    }"#;
    // When: parsed and re-serialized.
    let cfg = BudgetConfig::from_json_str(json).expect("override row parses");
    // Then: the override is present with its timeout.
    assert_eq!(
        cfg.media_format_overrides
            .get("avif")
            .map(|row| row.encode_timeout_ms),
        Some(30_000),
        "avif encode timeout is the contract of record"
    );
    let round =
        BudgetConfig::from_json_str(&cfg.to_json().expect("serialize")).expect("round-trip parses");
    assert_eq!(round, cfg, "the override survives a round trip");
}

#[test]
fn media_format_override_rows_are_strictly_typed() {
    // Given: an override row with an unknown key.
    let json = r#"{
        "cold_start_ms": 50,
        "idle_max_renders": 1,
        "rss_mb": 80,
        "p99_frame_ms": 16,
        "binary_mb": 40,
        "regression_tolerance_pct": 10,
        "frame_budget_ms": 10,
        "hang_threshold_ms": 100,
        "media_format_overrides": {
            "avif": {"encode_timeout_ms": 30000, "surprise": 1}
        }
    }"#;
    // When/Then: strict parse rejects it typed, never silently drops it.
    let err = BudgetConfig::from_json_str(json).expect_err("unknown override key");
    assert!(matches!(err, BudgetConfigError::Malformed(_)), "got: {err}");
}

#[test]
fn committed_budgets_json_parses_with_the_avif_override() {
    // The file of record stays parseable under the strict schema.
    let bytes = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("budgets.json"),
    )
    .expect("budgets.json present");
    let cfg = BudgetConfig::from_json_str(&bytes).expect("committed budgets parse");
    assert_eq!(
        cfg.media_format_overrides
            .get("avif")
            .map(|row| row.encode_timeout_ms),
        Some(30_000)
    );
}
