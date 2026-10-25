//! Gate evaluation: absolute budgets (todo 43), >10% regression against
//! committed baselines, scenario roll-up, exit-code mapping. Pure over the
//! measured numbers — the report always carries the MEASURED value.

use std::collections::BTreeSet;

use crate::report::{GateReport, MetricRecord, MetricStatus, ScenarioRecord};

pub const EXIT_PASS: i32 = 0;
pub const EXIT_BREACH: i32 = 2;
pub const EXIT_CONFIG: i32 = 3;

/// How a measured metric is judged. All metrics are lower-is-better.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Policy {
    /// Absolute ceiling from [`crate::budgets::BudgetConfig`]; strictly
    /// above breaches ("budget is an upper bound").
    Budget { budget: f64 },
    /// Deterministic metric compared against the committed baseline;
    /// strictly more than `tolerance_pct` over breaches.
    Regression,
    /// Recorded in the report, never gated (wall-clock trend numbers).
    Advisory,
}

/// `(current - baseline) / baseline * 100`, or `None` for a zero baseline.
pub fn regression_pct(current: f64, baseline: f64) -> Option<f64> {
    (baseline > 0.0).then(|| (current - baseline) / baseline * 100.0)
}

fn budget_status(value: f64, budget: f64) -> MetricStatus {
    if value > budget {
        MetricStatus::Breach
    } else {
        MetricStatus::Pass
    }
}

fn regression_status(value: f64, baseline: f64, tolerance_pct: u64) -> MetricStatus {
    match regression_pct(value, baseline) {
        Some(pct) if pct > tolerance_pct as f64 => MetricStatus::Breach,
        // Zero baseline: any positive current is an unbounded regression.
        None if value > 0.0 => MetricStatus::Breach,
        _ => MetricStatus::Pass,
    }
}

/// Judge one measured metric and produce its report record.
pub fn evaluate_metric(
    name: &str,
    unit: &str,
    value: f64,
    policy: Policy,
    baseline: Option<f64>,
    tolerance_pct: u64,
) -> MetricRecord {
    let (status, budget, regression) = match policy {
        Policy::Budget { budget } => (budget_status(value, budget), Some(budget), None),
        Policy::Regression => {
            let baseline = baseline.unwrap_or(0.0);
            (
                regression_status(value, baseline, tolerance_pct),
                None,
                regression_pct(value, baseline),
            )
        }
        Policy::Advisory => (MetricStatus::Pass, None, None),
    };
    MetricRecord {
        name: name.to_owned(),
        value,
        unit: unit.to_owned(),
        status,
        budget,
        baseline,
        regression_pct: regression,
        note: None,
    }
}

/// Roll metric records into a scenario record (breach dominates).
pub fn scenario_record(scenario: &str, metrics: Vec<MetricRecord>) -> ScenarioRecord {
    let status = if metrics.iter().any(|m| m.status == MetricStatus::Breach) {
        MetricStatus::Breach
    } else {
        MetricStatus::Pass
    };
    ScenarioRecord {
        scenario: scenario.to_owned(),
        status,
        metrics,
    }
}

/// 0 = all pass; 2 = any breach; 3 = a skipped scenario not explicitly
/// allowed (missing input is a gate error, never a silent pass).
pub fn exit_code(report: &GateReport, allowed_skips: &BTreeSet<String>) -> i32 {
    if report
        .scenarios
        .iter()
        .any(|s| s.status == MetricStatus::Breach)
    {
        return EXIT_BREACH;
    }
    let unallowed_skip = report
        .scenarios
        .iter()
        .any(|s| s.status == MetricStatus::Skipped && !allowed_skips.contains(&s.scenario));
    if unallowed_skip {
        EXIT_CONFIG
    } else {
        EXIT_PASS
    }
}
