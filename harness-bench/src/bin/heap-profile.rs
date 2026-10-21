//! heap-profile: archived dhat heap profiles + the allocation-count
//! regression gate (feature `dhat`). `--record` archives the current
//! run's per-scenario allocation totals; `--check` compares a fresh run
//! against the committed archive with the >10% tolerance.

#[cfg(feature = "dhat")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use harness_bench::baseline::Baseline;
use harness_bench::runner::{
    evaluate, gate_exit, load_budgets, record_baseline, run_named, write_report,
};
use harness_bench::scenarios::{HEAP_BASELINE_METRICS, REGISTRY, ScenarioContext};

const HEAP_SCENARIOS: &[&str] = &["repomap", "eviction", "prompt", "pty", "sessions", "tui"];

const EXIT_CONFIG: u8 = 3;

#[derive(Parser)]
#[command(
    name = "heap-profile",
    about = "dhat heap profiles + allocation regression gate (todo 43)"
)]
struct Args {
    /// Record this run's allocation metrics as the heap baseline
    #[arg(long)]
    record: Option<PathBuf>,
    /// Check against the committed heap baseline
    #[arg(long)]
    check: Option<PathBuf>,
    /// Write the JSON profile report here (with --check)
    #[arg(long)]
    report: Option<PathBuf>,
    /// Budgets of record (for the regression tolerance)
    #[arg(long)]
    budgets: Option<PathBuf>,
}

fn main() -> ExitCode {
    // HeapStats::get() panics without a live profiler; this one owns the
    // whole run (normal mode — no leak assertions at drop).
    #[cfg(feature = "dhat")]
    let _profiler = dhat::Profiler::builder().build();

    let args = Args::parse();
    if args.record.is_none() && args.check.is_none() {
        eprintln!("heap-profile: pass exactly one of --record or --check");
        return ExitCode::from(EXIT_CONFIG);
    }
    let (budgets, budgets_source) = match load_budgets(args.budgets.as_deref()) {
        Ok(loaded) => loaded,
        Err(err) => {
            eprintln!("heap-profile: {err}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    let regression_tolerance_pct = budgets.regression_tolerance_pct;
    let ctx = ScenarioContext {
        budgets,
        wall_runs: 1,
        ..ScenarioContext::for_tests()
    };
    let selected: Vec<(&str, _)> = REGISTRY
        .iter()
        .copied()
        .filter(|(name, _)| HEAP_SCENARIOS.contains(name))
        .collect();
    let outcome = run_named(&ctx, &selected);

    if let Some(path) = &args.record {
        match record_baseline(&outcome, HEAP_BASELINE_METRICS, path, std::env::consts::OS) {
            Ok(()) => {
                println!("heap-profile: recorded {}", path.display());
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("heap-profile: {err}");
                ExitCode::from(EXIT_CONFIG)
            }
        }
    } else {
        let default_path = PathBuf::from("harness-bench/baselines/heap_profile.json");
        let path = args.check.as_deref().unwrap_or(&default_path);
        let baseline = match std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|json| Baseline::from_json_str(&json).map_err(|e| e.to_string()))
        {
            Ok(baseline) => baseline,
            Err(err) => {
                eprintln!("heap-profile: {err}");
                return ExitCode::from(EXIT_CONFIG);
            }
        };
        match evaluate(
            &outcome,
            HEAP_BASELINE_METRICS,
            Some(&baseline),
            regression_tolerance_pct,
            &budgets_source,
        ) {
            Ok(report) => {
                println!("heap-profile: {}", report.summary());
                if let Some(report_path) = &args.report
                    && let Err(err) = write_report(&report, report_path)
                {
                    eprintln!("heap-profile: report not written: {err}");
                }
                let code = gate_exit(&report, &[]) as u8;
                ExitCode::from(code)
            }
            Err(err) => {
                eprintln!("heap-profile: {err}");
                ExitCode::from(EXIT_CONFIG)
            }
        }
    }
}
