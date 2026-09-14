//! Gate evaluation: absolute budgets, >10% regression vs baseline,
//! scenario roll-up, and exit-code mapping. The +50%-slowdown injection
//! proves breaches carry MEASURED numbers and fail the gate.

use std::collections::BTreeSet;

use harness_bench::gate::{
    EXIT_BREACH, EXIT_CONFIG, EXIT_PASS, Policy, evaluate_metric, exit_code, regression_pct,
    scenario_record,
};
use harness_bench::report::{GateReport, GitMeta, MetricStatus, ScenarioRecord};

fn report_from(scenarios: Vec<ScenarioRecord>) -> GateReport {
    let passed = scenarios
        .iter()
        .all(|s| s.status == MetricStatus::Pass || s.status == MetricStatus::Skipped);
    GateReport {
        passed,
        git: GitMeta::not_captured(),
        budgets_source: "test".to_owned(),
        scenarios,
    }
}

#[test]
fn budget_passes_at_exactly_the_ceiling() {
    let rec = evaluate_metric(
        "cold_start_ms",
        "ms",
        50.0,
        Policy::Budget { budget: 50.0 },
        None,
        10,
    );
    assert_eq!(
        rec.status,
        MetricStatus::Pass,
        "budget is an upper bound, 50.0 == 50 passes"
    );
    assert_eq!(rec.budget, Some(50.0));
}

#[test]
fn budget_breaches_above_the_ceiling() {
    let rec = evaluate_metric(
        "cold_start_ms",
        "ms",
        50.5,
        Policy::Budget { budget: 50.0 },
        None,
        10,
    );
    assert_eq!(rec.status, MetricStatus::Breach);
}

#[test]
fn injected_fifty_percent_slowdown_fails_with_measured_number() {
    // Failure QA channel: +50% slowdown (50ms budget -> 75ms measured).
    let rec = evaluate_metric(
        "cold_start_ms",
        "ms",
        75.0,
        Policy::Budget { budget: 50.0 },
        None,
        10,
    );
    assert_eq!(rec.status, MetricStatus::Breach);
    assert_eq!(
        rec.value, 75.0,
        "breach record carries the MEASURED value, not a constant"
    );
    assert_eq!(rec.budget, Some(50.0));
}

#[test]
fn regression_passes_at_exactly_ten_percent_and_fails_above() {
    let policy = Policy::Regression;
    let at_limit = evaluate_metric("m", "bytes", 110.0, policy, Some(100.0), 10);
    assert_eq!(
        at_limit.status,
        MetricStatus::Pass,
        ">10% means 110.0 exactly still passes"
    );
    let over = evaluate_metric("m", "bytes", 110.001, policy, Some(100.0), 10);
    assert_eq!(over.status, MetricStatus::Breach);
}

#[test]
fn regression_breach_reports_the_measured_regression_percent() {
    let rec = evaluate_metric("m", "bytes", 150.0, Policy::Regression, Some(100.0), 10);
    assert_eq!(rec.status, MetricStatus::Breach);
    let pct = rec.regression_pct.expect("breach carries regression pct");
    assert!((pct - 50.0).abs() < 1e-9, "pct={pct}");
    assert_eq!(rec.baseline, Some(100.0));
}

#[test]
fn zero_baseline_breaches_on_any_positive_current() {
    let zero = evaluate_metric("m", "count", 0.0, Policy::Regression, Some(0.0), 10);
    assert_eq!(zero.status, MetricStatus::Pass);
    let one = evaluate_metric("m", "count", 1.0, Policy::Regression, Some(0.0), 10);
    assert_eq!(
        one.status,
        MetricStatus::Breach,
        "0 -> positive is an unbounded regression"
    );
}

#[test]
fn advisory_metrics_are_recorded_but_never_gate() {
    let rec = evaluate_metric(
        "repomap_render_ms",
        "ms",
        123_456.0,
        Policy::Advisory,
        None,
        10,
    );
    assert_eq!(rec.status, MetricStatus::Pass);
    assert_eq!(rec.value, 123_456.0);
    assert_eq!(rec.budget, None);
}

#[test]
fn scenario_record_rolls_up_breach_status() {
    let pass = evaluate_metric(
        "renders",
        "count",
        1.0,
        Policy::Budget { budget: 1.0 },
        None,
        10,
    );
    let breach = evaluate_metric(
        "p99_frame_ms",
        "ms",
        20.0,
        Policy::Budget { budget: 16.0 },
        None,
        10,
    );
    let clean = scenario_record("tui", vec![pass.clone()]);
    assert_eq!(clean.status, MetricStatus::Pass);
    let dirty = scenario_record("tui2", vec![pass, breach]);
    assert_eq!(dirty.status, MetricStatus::Breach);
}

#[test]
fn regression_pct_helper_is_relative_to_baseline() {
    assert!((regression_pct(110.0, 100.0).expect("pct") - 10.0).abs() < 1e-9);
    assert_eq!(regression_pct(50.0, 0.0), None);
}

#[test]
fn exit_codes_distinguish_pass_breach_and_unallowed_skip() {
    let pass_record = scenario_record(
        "idle",
        vec![evaluate_metric(
            "renders",
            "count",
            1.0,
            Policy::Budget { budget: 1.0 },
            None,
            10,
        )],
    );
    let breach_record = scenario_record(
        "cold",
        vec![evaluate_metric(
            "cold_start_ms",
            "ms",
            75.0,
            Policy::Budget { budget: 50.0 },
            None,
            10,
        )],
    );
    let skip_record = ScenarioRecord {
        scenario: "rss".to_owned(),
        status: MetricStatus::Skipped,
        metrics: vec![],
    };

    assert_eq!(
        exit_code(&report_from(vec![pass_record.clone()]), &BTreeSet::new()),
        EXIT_PASS
    );
    assert_eq!(
        exit_code(&report_from(vec![breach_record.clone()]), &BTreeSet::new()),
        EXIT_BREACH
    );
    assert_eq!(
        exit_code(&report_from(vec![skip_record.clone()]), &BTreeSet::new()),
        EXIT_CONFIG
    );
    let allowed: BTreeSet<String> = ["rss".to_owned()].into_iter().collect();
    assert_eq!(
        exit_code(&report_from(vec![skip_record]), &allowed),
        EXIT_PASS
    );
    assert_eq!(
        exit_code(&report_from(vec![breach_record, pass_record]), &allowed),
        EXIT_BREACH
    );
}
