//! heap-profile binary e2e (dhat feature only): record then check must
//! pass, and an inflated baseline must breach — allocation counts are
//! MEASURED, deterministic numbers.

#![cfg(feature = "dhat")]

use assert_cmd::Command;
use predicates::prelude::*;

fn profiler() -> Command {
    Command::cargo_bin("heap-profile").expect("heap-profile binary built")
}

#[test]
fn record_then_check_round_trips_green() {
    let dir = tempfile::tempdir().expect("tempdir");
    let baseline = dir.path().join("heap.json");
    let baseline_arg = baseline.to_str().expect("utf8").to_owned();
    profiler()
        .arg("--record")
        .arg(&baseline_arg)
        .assert()
        .success()
        .stdout(predicate::str::contains("recorded"));
    profiler()
        .arg("--check")
        .arg(&baseline_arg)
        .assert()
        .success()
        .code(0);
}

#[test]
fn shrunken_baseline_breaches_the_regression_gate() {
    // Baseline at 1/10 of real allocation counts: current is ~+900%,
    // far past the 10% tolerance — must exit 2 with the metric named.
    let dir = tempfile::tempdir().expect("tempdir");
    let honest = dir.path().join("honest.json");
    let honest_arg = honest.to_str().expect("utf8").to_owned();
    profiler()
        .arg("--record")
        .arg(&honest_arg)
        .assert()
        .success();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&honest).expect("read")).expect("parse");
    let mut inflated = json.clone();
    let metrics = inflated["metrics"].as_object_mut().expect("metrics object");
    for (_name, value) in metrics.iter_mut() {
        let shrunk = value.as_f64().expect("numeric") / 10.0;
        *value = serde_json::json!(shrunk);
    }
    let shrunk_path = dir.path().join("shrunk.json");
    std::fs::write(&shrunk_path, inflated.to_string()).expect("write shrunk baseline");
    profiler()
        .arg("--check")
        .arg(shrunk_path.to_str().expect("utf8"))
        .assert()
        .failure()
        .code(2);
}
