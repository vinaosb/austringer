//! Gate report artifacts: measured numbers per scenario, verdicts, and git
//! provenance that DISTINGUISHES clean vs dirty worktrees.

use harness_bench::report::{
    GateReport, GitMeta, MetricRecord, MetricStatus, ScenarioRecord, parse_porcelain,
};

#[test]
fn porcelain_output_distinguishes_clean_from_dirty() {
    assert!(!parse_porcelain(""), "empty status = clean");
    assert!(!parse_porcelain("\n\n"), "blank lines = clean");
    assert!(parse_porcelain(" M Cargo.toml\n"), "modified = dirty");
    assert!(parse_porcelain("?? new-file\n"), "untracked = dirty");
    assert!(parse_porcelain("M  staged\n"), "staged = dirty");
}

#[test]
fn uncaptured_git_meta_is_explicit_not_silent() {
    let meta = GitMeta::not_captured();
    assert!(!meta.captured);
    assert_eq!(meta.head, None);
    assert!(!meta.dirty, "uncaptured must never claim dirty either way");
}

#[test]
fn report_round_trips_through_json() {
    let report = GateReport {
        passed: false,
        git: GitMeta {
            captured: true,
            head: Some("238ad5e".to_owned()),
            dirty: true,
        },
        budgets_source: "harness-bench/budgets.json".to_owned(),
        scenarios: vec![ScenarioRecord {
            scenario: "cold_start".to_owned(),
            status: MetricStatus::Breach,
            metrics: vec![MetricRecord {
                name: "cold_start_ms".to_owned(),
                value: 75.0,
                unit: "ms".to_owned(),
                status: MetricStatus::Breach,
                budget: Some(50.0),
                baseline: None,
                regression_pct: None,
                note: Some("min of 7 spawns".to_owned()),
            }],
        }],
    };
    let json = report.to_json_pretty().expect("serialize");
    let back: GateReport = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, report);
    // Breached metric renders its MEASURED value (75), not the budget.
    assert!(json.contains("\"value\": 75.0"), "json: {json}");
}

#[test]
fn summary_line_counts_breaches_and_scenarios() {
    let report = GateReport {
        passed: false,
        git: GitMeta::not_captured(),
        budgets_source: "defaults".to_owned(),
        scenarios: vec![
            ScenarioRecord {
                scenario: "idle".to_owned(),
                status: MetricStatus::Pass,
                metrics: vec![],
            },
            ScenarioRecord {
                scenario: "cold_start".to_owned(),
                status: MetricStatus::Breach,
                metrics: vec![MetricRecord {
                    name: "cold_start_ms".to_owned(),
                    value: 75.0,
                    unit: "ms".to_owned(),
                    status: MetricStatus::Breach,
                    budget: Some(50.0),
                    baseline: None,
                    regression_pct: None,
                    note: None,
                }],
            },
        ],
    };
    let summary = report.summary();
    assert!(summary.contains("2 scenarios"), "summary: {summary}");
    assert!(summary.contains("1 breach"), "summary: {summary}");
}
