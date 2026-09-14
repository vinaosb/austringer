//! End-to-end proof of the perf-gate binary: real measurement paths,
//! typed config errors, non-zero breach exits, and reports carrying
//! MEASURED numbers.

use assert_cmd::Command;
use predicates::prelude::*;

fn gate() -> Command {
    Command::cargo_bin("perf-gate").expect("perf-gate binary built")
}

#[test]
fn self_test_measures_a_real_breach_and_exits_two() {
    let dir = tempfile::tempdir().expect("tempdir");
    let report = dir.path().join("report.json");
    let report_arg = report.to_str().expect("utf8 path").to_owned();
    gate()
        .arg("--self-test")
        .arg("--report")
        .arg(&report_arg)
        .assert()
        .failure()
        .code(2)
        .stderr(
            predicate::str::contains("BREACH")
                .and(predicate::str::contains("self_test.cold_start_ms")),
        );
    let json = std::fs::read_to_string(&report).expect("report written");
    assert!(
        json.contains("\"value\""),
        "report carries measured values: {json}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid report json");
    let measured = parsed["scenarios"][0]["metrics"][0]["value"]
        .as_f64()
        .expect("numeric measured value");
    assert!(
        measured >= 150.0,
        "self-test measures >=3x50ms, got {measured}"
    );
    assert_eq!(parsed["scenarios"][0]["metrics"][0]["status"], "breach");
}

#[test]
fn missing_budgets_file_is_a_config_error() {
    gate()
        .arg("--self-test")
        .arg("--budgets")
        .arg("Z:/definitely/not/here.json")
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("budgets file"));
}

#[test]
fn malformed_budgets_file_is_a_typed_config_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bad = dir.path().join("budgets.json");
    std::fs::write(&bad, "{ not json").expect("write bad budgets");
    gate()
        .arg("--self-test")
        .arg("--budgets")
        .arg(bad.to_str().expect("utf8"))
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("malformed budgets config"));
}

#[test]
fn invalid_binary_spec_is_a_config_error() {
    gate()
        .arg("--self-test")
        .arg("--binary")
        .arg("no-equals-sign")
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("NAME=PATH"));
}

#[test]
fn regression_metrics_without_baseline_are_a_config_error() {
    // No --baseline and no --self-test: the deterministic scenarios emit
    // regression metrics, and ungated regressions must never pass.
    gate()
        .arg("--wall-runs")
        .arg("1")
        .arg("--allow-skip")
        .arg("cold_start")
        .arg("--allow-skip")
        .arg("rss")
        .arg("--allow-skip")
        .arg("binary_size")
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("--baseline"));
}

#[test]
fn full_run_with_recorded_baseline_passes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let baseline = dir.path().join("baseline.json");
    let baseline_arg = baseline.to_str().expect("utf8").to_owned();
    let skips = ["cold_start", "rss", "binary_size"];
    let mut record = gate();
    record
        .arg("--record-baseline")
        .arg("--baseline")
        .arg(&baseline_arg)
        .arg("--wall-runs")
        .arg("1");
    for skip in skips {
        record.arg("--allow-skip").arg(skip);
    }
    record
        .assert()
        .success()
        .stdout(predicate::str::contains("recorded"));

    let report = dir.path().join("report.json");
    let mut check = gate();
    check
        .arg("--baseline")
        .arg(&baseline_arg)
        .arg("--report")
        .arg(report.to_str().expect("utf8"))
        .arg("--wall-runs")
        .arg("1");
    for skip in skips {
        check.arg("--allow-skip").arg(skip);
    }
    check.assert().success().code(0).stdout(
        predicate::str::contains("3 skipped").and(predicate::str::contains("verdict PASS")),
    );
    let json = std::fs::read_to_string(&report).expect("report written");
    assert!(
        json.contains("repomap.tokens"),
        "report has per-scenario numbers: {json}"
    );
}

#[test]
fn stale_baseline_is_a_config_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stale = dir.path().join("stale.json");
    std::fs::write(
        &stale,
        r#"{"schema": 1, "recorded_on": null, "metrics": {"repomap.tokens": 900.0}}"#,
    )
    .expect("write stale baseline");
    gate()
        .arg("--baseline")
        .arg(stale.to_str().expect("utf8"))
        .arg("--wall-runs")
        .arg("1")
        .arg("--allow-skip")
        .arg("cold_start")
        .arg("--allow-skip")
        .arg("rss")
        .arg("--allow-skip")
        .arg("binary_size")
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("stale baseline"));
}
