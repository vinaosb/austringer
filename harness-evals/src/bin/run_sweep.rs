//! run_sweep: the todo-26 hermetic conformance sweep driver. Runs
//! the committed scenario pack (fake provider, zero network, zero
//! API keys) through the evals machinery and writes the report
//! artifacts under docs/release/ (the evals.adoc publication source).
//!
//! Exit codes (perf-gate convention): 0 = every pinned mechanism
//! conforms and no unpinned regression; 2 = breach (golden-prefix
//! drift or an unpinned regression — surfaced loudly, never
//! absorbed); 3 = config error (bad args, unreadable pack, corrupted
//! cassette — the cassette is named — or a failed git source probe).
//!
//! Live-model sweeps are NOT this bin's job: they stay scheduled-only
//! under the daemon's cron (todo 41) and never gate PRs.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use harness_evals::driver::{self, DriverArgs};
use harness_evals::report::SourceFingerprint;

const EXIT_CONFIG: u8 = 3;

struct Cli {
    conformance: bool,
    trigger: String,
    fixtures: Option<PathBuf>,
    report: Option<PathBuf>,
}

fn parse_args() -> Result<Cli, String> {
    let mut cli = Cli {
        conformance: false,
        trigger: "manual".to_owned(),
        fixtures: None,
        report: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--conformance" => cli.conformance = true,
            "--trigger" => {
                let value = args.next().ok_or("--trigger needs a value")?;
                cli.trigger = value;
            }
            "--fixtures" => {
                let value = args.next().ok_or("--fixtures needs a directory")?;
                cli.fixtures = Some(PathBuf::from(value));
            }
            "--report" => {
                let value = args.next().ok_or("--report needs a file")?;
                cli.report = Some(PathBuf::from(value));
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if !cli.conformance {
        return Err("only the --conformance sweep is implemented".to_owned());
    }
    Ok(cli)
}

fn probe_source(root: &Path) -> Result<SourceFingerprint, String> {
    let run = |args: &[&str]| -> Result<String, String> {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| format!("git {} failed: {e}", args[0]))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(format!(
                "git {} failed with {}: {stderr}",
                args[0], output.status
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let commit = run(&["rev-parse", "--short=12", "HEAD"])?.trim().to_owned();
    let dirty_files: Vec<String> = run(&["status", "--porcelain"])?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    Ok(SourceFingerprint {
        commit: Some(commit),
        dirty: !dirty_files.is_empty(),
        dirty_files,
    })
}

fn main() -> ExitCode {
    let cli = match parse_args() {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("run_sweep: {err}");
            eprintln!(
                "usage: run_sweep --conformance [--trigger scheduled|manual] \
                 [--fixtures DIR] [--report FILE]"
            );
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = manifest.parent() else {
        eprintln!("run_sweep: cannot resolve the workspace root from {manifest:?}");
        return ExitCode::from(EXIT_CONFIG);
    };
    let args = DriverArgs {
        trigger: match driver::parse_trigger(&cli.trigger) {
            Ok(trigger) => trigger,
            Err(err) => {
                eprintln!("run_sweep: {err}");
                return ExitCode::from(EXIT_CONFIG);
            }
        },
        fixtures: cli.fixtures.unwrap_or_else(|| manifest.join("scenarios")),
        report: cli.report.unwrap_or_else(|| {
            workspace_root
                .join("docs")
                .join("release")
                .join("evals-conformance-report.json")
        }),
    };
    let source = match probe_source(workspace_root) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("run_sweep: source probe failed: {err}");
            return ExitCode::from(EXIT_CONFIG);
        }
    };
    match driver::run(&args, &source) {
        Ok(verdict) => {
            for finding in &verdict.drift {
                eprintln!("CONFORMANCE DRIFT: {finding}");
            }
            let regressions = verdict.report.pins.unpinned.len();
            eprintln!(
                "conformance: {} pinned cases, {} drift finding(s), {regressions} unpinned regression(s)",
                harness_evals::scenarios::PINNED_CASES.len(),
                verdict.drift.len(),
            );
            print!("{}", verdict.table);
            println!("artifact: {}", args.report.display());
            ExitCode::from(verdict.exit_code())
        }
        Err(err) => {
            eprintln!("run_sweep: {err}");
            ExitCode::from(err.exit_code())
        }
    }
}
