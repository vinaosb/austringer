//! perf-gate: the todo-43 CI gate. Runs the perf scenarios, evaluates the
//! budgets of record plus >10% regressions against committed
//! machine-independent baselines, writes a JSON report with MEASURED
//! numbers, and exits 0 (pass) / 2 (breach) / 3 (config error).
//!
//! Noise-margin discipline: wall-clock budgets use min-of-N sampling and
//! generous ceilings; ONLY deterministic counters (tokens/bytes/events)
//! are baseline-regression-gated, so the verdict is stable on loaded
//! machines and comparable across runners.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use harness_bench::baseline::Baseline;
use harness_bench::report::GateReport;
use harness_bench::runner::{
    evaluate, gate_exit, load_budgets, record_baseline, run_named, write_report,
};
use harness_bench::scenarios::{GATE_BASELINE_METRICS, REGISTRY, ScenarioContext};

const EXIT_CONFIG: u8 = 3;

#[derive(Parser)]
#[command(
    name = "perf-gate",
    about = "harness perf budgets + regression gate (todo 43)"
)]
struct Args {
    /// Budgets of record (JSON); omit for the plan todo-43 defaults
    #[arg(long)]
    budgets: Option<PathBuf>,
    /// Committed deterministic baseline (regression gate)
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Write the JSON gate report here
    #[arg(long)]
    report: Option<PathBuf>,
    /// CLI binary for the cold-start budget (target/release/harness)
    #[arg(long)]
    cli_bin: Option<PathBuf>,
    /// Daemon binary for the RSS budget (target/release/harness-daemon)
    #[arg(long)]
    daemon_bin: Option<PathBuf>,
    /// Named binary-size target NAME=PATH (repeatable)
    #[arg(long = "binary", value_name = "NAME=PATH")]
    binaries: Vec<String>,
    /// Explicitly allow a skipped scenario (recorded, not fatal)
    #[arg(long = "allow-skip")]
    allow_skip: Vec<String>,
    /// Record this run's deterministic metrics as the gate baseline
    #[arg(long)]
    record_baseline: bool,
    /// End-to-end breach-detection self test (synthetic slow scenario)
    #[arg(long)]
    self_test: bool,
    /// Min-of-N sample count for wall measurements
    #[arg(long, default_value_t = 7)]
    wall_runs: u32,
}

fn parse_binary(spec: &str) -> Result<(String, PathBuf), String> {
    let (name, path) = spec
        .split_once('=')
        .ok_or_else(|| format!("--binary expects NAME=PATH, got {spec:?}"))?;
    if name.is_empty() || path.is_empty() {
        return Err(format!("--binary expects NAME=PATH, got {spec:?}"));
    }
    Ok((name.to_owned(), PathBuf::from(path)))
}

fn main() -> ExitCode {
    // When built with the dhat feature, scenarios snapshot heap stats;
    // keep a live (normal-mode) profiler for the whole run.
    #[cfg(feature = "dhat")]
    let _profiler = dhat::Profiler::builder().build();

    let args = Args::parse();
    let (budgets, budgets_source) = match load_budgets(args.budgets.as_deref()) {
        Ok(loaded) => loaded,
        Err(err) => {
            eprintln!("perf-gate: {err}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    let binaries = match args
        .binaries
        .iter()
        .map(|s| parse_binary(s))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(binaries) => binaries,
        Err(err) => {
            eprintln!("perf-gate: {err}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    let ctx = ScenarioContext {
        budgets,
        cli_bin: args.cli_bin.clone(),
        daemon_bin: args.daemon_bin.clone(),
        binaries,
        wall_runs: args.wall_runs,
    };

    let selected: Vec<(&str, _)> = if args.self_test {
        REGISTRY
            .iter()
            .copied()
            .filter(|(name, _)| *name == "self_test")
            .collect()
    } else {
        REGISTRY
            .iter()
            .copied()
            .filter(|(name, _)| *name != "self_test")
            .collect()
    };
    let outcome = run_named(&ctx, &selected);

    if args.record_baseline {
        let default_path = PathBuf::from("harness-bench/baselines/gate_baseline.json");
        let path = args.baseline.as_deref().unwrap_or(&default_path);
        if let Err(err) =
            record_baseline(&outcome, GATE_BASELINE_METRICS, path, std::env::consts::OS)
        {
            eprintln!("perf-gate: {err}");
            return ExitCode::from(EXIT_CONFIG);
        }
        println!("perf-gate: baseline recorded to {}", path.display());
        return ExitCode::SUCCESS;
    }

    let needs_baseline = outcome.outputs.values().any(|o| {
        o.metrics
            .iter()
            .any(|m| m.policy == harness_bench::gate::Policy::Regression)
    });
    let baseline = match load_baseline(&args.baseline) {
        Ok(baseline) => baseline,
        Err(err) => {
            eprintln!("perf-gate: {err}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    if needs_baseline && baseline.is_none() && !args.self_test {
        eprintln!(
            "perf-gate: regression metrics were produced but no --baseline was given \
             (use --record-baseline to create one)"
        );
        return ExitCode::from(EXIT_CONFIG);
    }

    let report = match evaluate(
        &outcome,
        GATE_BASELINE_METRICS,
        baseline.as_ref(),
        budgets.regression_tolerance_pct,
        &budgets_source,
    ) {
        Ok(report) => report,
        Err(err) => {
            eprintln!("perf-gate: {err}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    let code = gate_exit(&report, &args.allow_skip) as u8;
    finish(&report, &args.report);
    ExitCode::from(code)
}

fn load_baseline(path: &Option<PathBuf>) -> Result<Option<Baseline>, String> {
    match path {
        None => Ok(None),
        Some(path) => {
            let json = std::fs::read_to_string(path)
                .map_err(|e| format!("baseline {}: {e}", path.display()))?;
            Baseline::from_json_str(&json)
                .map(Some)
                .map_err(|e| format!("baseline {}: {e}", path.display()))
        }
    }
}

fn finish(report: &GateReport, report_path: &Option<PathBuf>) {
    println!("perf-gate: {}", report.summary());
    for scenario in &report.scenarios {
        if scenario.status == harness_bench::report::MetricStatus::Breach {
            eprintln!("perf-gate: BREACH in {}:", scenario.scenario);
            for metric in &scenario.metrics {
                if metric.status == harness_bench::report::MetricStatus::Breach {
                    eprintln!(
                        "  {} = {} {} (limit {:?}, baseline {:?}, regression {:?})",
                        metric.name,
                        metric.value,
                        metric.unit,
                        metric.budget,
                        metric.baseline,
                        metric.regression_pct
                    );
                }
            }
        }
    }
    if let Some(path) = report_path
        && let Err(err) = write_report(report, path)
    {
        eprintln!("perf-gate: report not written: {err}");
    }
}
