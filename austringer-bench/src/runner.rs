//! Scenario execution + evaluation glue shared by the perf-gate and
//! heap-profile binaries (and their tests).

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use crate::baseline::{BASELINE_SCHEMA, Baseline, BaselineError};
use crate::budgets::BudgetConfig;
use crate::gate::{EXIT_CONFIG, Policy, evaluate_metric, exit_code, scenario_record};
use crate::report::{GateReport, GitMeta, MetricRecord, MetricStatus, ScenarioRecord};
use crate::scenarios::{
    ScenarioContext, ScenarioError, ScenarioFn, ScenarioOutput, deterministic_metrics,
};

pub struct RunOutcome {
    pub records: Vec<ScenarioRecord>,
    pub outputs: BTreeMap<String, ScenarioOutput>,
}

/// Run the named scenarios; skips and engine failures become loud
/// records (a failed measurement can never pass the gate silently).
pub fn run_named(ctx: &ScenarioContext, names: &[(&str, ScenarioFn)]) -> RunOutcome {
    let mut records = Vec::new();
    let mut outputs = BTreeMap::new();
    for (name, run) in names {
        match run(ctx) {
            Ok(output) => {
                outputs.insert((*name).to_owned(), output);
            }
            Err(ScenarioError::Skipped(why)) => records.push(ScenarioRecord {
                scenario: (*name).to_owned(),
                status: MetricStatus::Skipped,
                metrics: vec![skip_note(why)],
            }),
            Err(ScenarioError::Failed(why)) => {
                let mut record = ScenarioRecord {
                    scenario: (*name).to_owned(),
                    status: MetricStatus::Breach,
                    metrics: vec![],
                };
                record.metrics.push(MetricRecord {
                    name: format!("{name}.run"),
                    value: 1.0,
                    unit: "error".to_owned(),
                    status: MetricStatus::Breach,
                    budget: Some(0.0),
                    baseline: None,
                    regression_pct: None,
                    note: Some(why),
                });
                records.push(record);
            }
        }
    }
    RunOutcome { records, outputs }
}

fn skip_note(why: String) -> MetricRecord {
    MetricRecord {
        name: "skipped".to_owned(),
        value: 0.0,
        unit: "n/a".to_owned(),
        status: MetricStatus::Skipped,
        budget: None,
        baseline: None,
        regression_pct: None,
        note: Some(why),
    }
}

/// Evaluate a run into a gate report. Regression-policy metrics are
/// compared against `baseline` (scoped: every produced metric must be in
/// the baseline; baseline keys outside `registry` are stale).
pub fn evaluate(
    outcome: &RunOutcome,
    registry: &[&str],
    baseline: Option<&Baseline>,
    tolerance_pct: u64,
    budgets_source: &str,
) -> Result<GateReport, BaselineError> {
    let registry_set: BTreeSet<String> = registry.iter().map(|s| (*s).to_owned()).collect();
    if let Some(baseline) = baseline {
        let produced: BTreeSet<String> = outcome
            .outputs
            .values()
            .flat_map(|o| o.metrics.iter().map(|m| m.name.clone()))
            .filter(|name| registry_set.contains(name))
            .collect();
        baseline.check_scope(&registry_set, &produced)?;
    }
    let mut records = outcome.records.clone();
    for (name, output) in &outcome.outputs {
        let mut metrics = Vec::new();
        for metric in &output.metrics {
            let record = match metric.policy {
                // Out-of-registry regression metrics (e.g. allocator
                // counts leaking into a non-dhat run) stay visible but
                // are owned by the heap-profile gate, not this one.
                Policy::Regression if registry_set.contains(&metric.name) => {
                    let base = baseline.and_then(|b| b.get(&metric.name));
                    evaluate_metric(
                        &metric.name,
                        metric.unit,
                        metric.value,
                        Policy::Regression,
                        base,
                        tolerance_pct,
                    )
                }
                Policy::Regression => {
                    let mut record = evaluate_metric(
                        &metric.name,
                        metric.unit,
                        metric.value,
                        Policy::Advisory,
                        None,
                        0,
                    );
                    record.note =
                        Some("gated by the heap-profile run, not this baseline".to_owned());
                    record
                }
                policy => evaluate_metric(&metric.name, metric.unit, metric.value, policy, None, 0),
            };
            metrics.push(record);
        }
        records.push(scenario_record(name, metrics));
    }
    let passed = records.iter().all(|r| r.status != MetricStatus::Breach);
    Ok(GateReport {
        passed,
        git: GitMeta::capture(),
        budgets_source: budgets_source.to_owned(),
        scenarios: records,
    })
}

/// Write `outcome`'s registry metrics as a baseline artifact. Requires
/// every registry metric to have been produced (no partial baselines).
pub fn record_baseline(
    outcome: &RunOutcome,
    registry: &[&str],
    path: &Path,
    recorded_on: &str,
) -> Result<(), String> {
    let metrics = deterministic_metrics(&outcome.outputs, registry);
    let missing: Vec<&str> = registry
        .iter()
        .filter(|m| !metrics.contains_key(**m))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "cannot record partial baseline, missing {missing:?}"
        ));
    }
    let baseline = Baseline {
        schema: BASELINE_SCHEMA,
        recorded_on: Some(recorded_on.to_owned()),
        metrics,
    };
    let json = baseline.to_json_pretty().map_err(|e| e.to_string())?;
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut writer = std::io::BufWriter::new(file);
    writer
        .write_all(json.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Write the JSON report artifact (best-effort: reported, not fatal).
pub fn write_report(report: &GateReport, path: &Path) -> Result<(), String> {
    let json = report.to_json_pretty().map_err(|e| e.to_string())?;
    std::fs::write(path, json.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Exit code for a report; skipped scenarios allowed via `allow_skip`.
pub fn gate_exit(report: &GateReport, allow_skip: &[String]) -> i32 {
    let allowed: BTreeSet<String> = allow_skip.iter().cloned().collect();
    exit_code(report, &allowed)
}

pub const CONFIG_EXIT: i32 = EXIT_CONFIG;

/// Budgets from an optional file; typed errors as strings for bin exit 3.
pub fn load_budgets(path: Option<&Path>) -> Result<(BudgetConfig, String), String> {
    match path {
        None => Ok((BudgetConfig::plan_defaults(), "plan-defaults".to_owned())),
        Some(path) => {
            let json = std::fs::read_to_string(path)
                .map_err(|e| format!("budgets file {}: {e}", path.display()))?;
            let config = BudgetConfig::from_json_str(&json).map_err(|e| e.to_string())?;
            Ok((config, path.display().to_string()))
        }
    }
}
