//! The `run_tests` tool (todo 17, ecosystem alignment): framework
//! detection + structured results. Day-one detection is cargo
//! (`cargo nextest` when installed, else `cargo test`); a configurable
//! command override extends the EXISTING [`crate::config::DaemonConfig`]
//! shape (`test_command`, user-global via `HARNESS_TEST_COMMAND`) — no
//! new config format, and a repo-local override file is refused with a
//! typed error naming the file. Execution rides the executor-#3 lane:
//! the todo-4 safety gate applies verbatim (a network-shaped override
//! is blocked before spawn), the env scrub drops KEY/SECRET/TOKEN at
//! the spawn seam, and the child lives in a kernel kill-tree domain.
//! Results parse into a typed [`TestReport`] — per-test
//! pass/fail/ignored + durations — never raw text.
//!
//! Permission tier: run_tests executes build scripts and test binaries
//! (arbitrary code), so it rides the SAME approval stage as
//! `execute_command`: no capability column maps it (no matrix toggle
//! can allow it) and under the default policy every call asks — it is
//! NEVER auto-allowed.
//!
//! Receipts: Zed #5242 (886 upvotes — the most-requested editor
//! integration); Qwen Test Failure Explorer (shipped 2026-10); JetBrains
//! native test runner.

mod parse;
mod report;
mod run;

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use harness_core::tools::{ProviderError, ToolSchema};

use super::providers::{optional_u64_arg, shell_argv, str_arg};
use super::safety::{SafetyDecision, gate, render_block, render_report};

pub use report::{TestEntry, TestReport, TestStatus, TestSummary};
use run::{RunError, RunOutcome, run_gated, stderr_head};

/// Default run ceiling (test suites compile; slower than an echo).
const DEFAULT_TEST_TIMEOUT_MS: u64 = 600_000;
/// The nextest-availability probe ceiling.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// The model-facing test runner. One instance per daemon, constructed
/// with the daemon's working root and the user-global command override.
#[derive(Clone)]
pub struct RunTestsTool {
    root: PathBuf,
    test_command: Option<String>,
}

impl RunTestsTool {
    /// Tool over `root` (the daemon's working directory) with the
    /// user-global override (`None` = framework detection).
    #[must_use]
    pub fn new(root: PathBuf, test_command: Option<String>) -> Self {
        Self { root, test_command }
    }

    fn resolve_root(&self, arguments: &Value) -> Result<PathBuf, ProviderError> {
        Ok(match arguments.get("path") {
            None | Some(Value::Null) => self.root.clone(),
            Some(_) => self.root.join(str_arg(arguments, "path")?),
        })
    }
}

#[async_trait]
impl harness_core::tools::ToolProvider for RunTestsTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "run_tests".into(),
            description: "Run the workspace's tests and return a structured TestReport (per-test pass/fail/ignored + durations + summary counts) — no output parsing needed. Detects cargo (cargo nextest when installed, else cargo test); a custom command can be set user-globally via HARNESS_TEST_COMMAND (repo-local override files are refused).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Crate or workspace root to test (default: the daemon's working directory)."
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Optional timeout ceiling in milliseconds (default 600000)."
                    }
                }
            }),
        }
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        let root = self.resolve_root(arguments)?;
        let timeout_ms =
            optional_u64_arg(arguments, "timeout_ms")?.unwrap_or(DEFAULT_TEST_TIMEOUT_MS);
        let timeout =
            Duration::from_millis(timeout_ms).min(harness_exec::spec::MAX_COMMAND_TIMEOUT);
        let test_command = self.test_command.clone();
        let bytes =
            tokio::task::spawn_blocking(move || run_sync(&root, test_command.as_deref(), timeout))
                .await
                .map_err(|join| ProviderError::Execution {
                    message: format!("run_tests task failed: {join}"),
                })?
                .map_err(|error: RunTestsError| ProviderError::Execution {
                    message: error.to_string(),
                })?;
        Ok(bytes)
    }
}

/// One full run: repo-local guard, framework decision, safety gate,
/// gated execution, parse, typed report.
fn run_sync(
    root: &Path,
    test_command: Option<&str>,
    timeout: Duration,
) -> Result<Vec<u8>, RunTestsError> {
    let repo_local = root.join(".harness").join("run_tests.json");
    if repo_local.is_file() {
        return Err(RunTestsError::RepoLocalOverride {
            file: repo_local.display().to_string().replace('\\', "/"),
        });
    }
    let (framework, program, args, gate_line) = match test_command {
        Some(command_line) => {
            // The gate verdicts the command line AS GIVEN, before shell
            // resolution — exactly where execute_command's gate sits
            // (the egress guard matches program words like `curl`, not
            // the shell wrapper around them).
            let (program, args) = shell_argv(command_line).map_err(|error| {
                RunTestsError::Run(RunError::Spawn {
                    program: "default shell".to_owned(),
                    detail: error.to_string(),
                })
            })?;
            (
                "override".to_owned(),
                program,
                args,
                command_line.to_owned(),
            )
        }
        None => {
            if !root.join("Cargo.toml").is_file() {
                return Err(RunTestsError::UnknownFramework {
                    searched: root.display().to_string().replace('\\', "/"),
                });
            }
            let cargo = cargo_program();
            if nextest_available(&cargo, root) {
                (
                    "cargo-nextest".to_owned(),
                    cargo.clone(),
                    vec!["nextest".into(), "run".into(), "--no-fail-fast".into()],
                    argv_display(&cargo, &["nextest".into(), "run".into(), "--no-fail-fast".into()]),
                )
            } else {
                (
                    "cargo-libtest".to_owned(),
                    cargo.clone(),
                    vec!["test".into()],
                    argv_display(&cargo, &["test".into()]),
                )
            }
        }
    };
    let command_line = gate_line.clone();
    match gate(&gate_line) {
        SafetyDecision::Block { violation, backend } => {
            Ok(render_block(&violation, backend).into_bytes())
        }
        SafetyDecision::Pass(verdict) => {
            let outcome = run_gated(&verdict, &program, &args, root, timeout)?;
            let tests = parse_results(&framework, &outcome);
            if tests.is_empty() && outcome.exit_code != Some(0) {
                return Err(RunTestsError::RunFailed {
                    command: command_line,
                    exit_code: outcome.exit_code,
                    stderr_head: stderr_head(&outcome.stderr),
                });
            }
            let report = TestReport {
                framework,
                command: command_line,
                exit_code: outcome.exit_code,
                timed_out: false,
                suite_duration_ms: outcome.elapsed.as_millis() as u64,
                summary: TestSummary::of(&tests),
                tests,
            };
            let json = serde_json::to_string(&report).map_err(|error| RunTestsError::Internal {
                detail: error.to_string(),
            })?;
            Ok(format!("{json}\n{}", render_report(&verdict)).into_bytes())
        }
    }
}

/// $CARGO points at the exact cargo binary when we ourselves run under
/// cargo (tests, CI); production resolves from PATH.
fn cargo_program() -> String {
    std::env::var("CARGO")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "cargo".to_owned())
}

fn argv_display(program: &str, args: &[String]) -> String {
    let mut line = program.to_owned();
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    line
}

/// nextest probe: a short gated `cargo nextest --version`. Any failure
/// (missing subcommand, spawn error) falls back to `cargo test` — if
/// cargo itself is broken, the real run surfaces the typed spawn error.
fn nextest_available(cargo: &str, root: &Path) -> bool {
    let probe_line = argv_display(cargo, &["nextest".into(), "--version".into()]);
    let SafetyDecision::Pass(verdict) = gate(&probe_line) else {
        return false;
    };
    matches!(
        run_gated(
            &verdict,
            cargo,
            &["nextest".into(), "--version".into()],
            root,
            PROBE_TIMEOUT
        ),
        Ok(outcome) if outcome.exit_code == Some(0)
    )
}

fn parse_results(framework: &str, outcome: &RunOutcome) -> Vec<TestEntry> {
    let nextest_lines = || {
        non_empty_or(parse::parse_nextest_human(&outcome.stderr), || {
            parse::parse_nextest_human(&outcome.stdout)
        })
    };
    let libtest_lines = || {
        non_empty_or(parse::parse_libtest(&outcome.stdout), || {
            parse::parse_libtest(&outcome.stderr)
        })
    };
    match framework {
        "cargo-nextest" => nextest_lines(),
        "override" => non_empty_or(nextest_lines(), libtest_lines),
        _ => libtest_lines(),
    }
}

fn non_empty_or(
    entries: Vec<TestEntry>,
    fallback: impl FnOnce() -> Vec<TestEntry>,
) -> Vec<TestEntry> {
    if entries.is_empty() {
        fallback()
    } else {
        entries
    }
}

/// Typed run_tests failures; every message carries what to do next.
#[derive(Debug, thiserror::Error)]
enum RunTestsError {
    /// A repo-local override file exists — refused; overrides are
    /// daemon-owned user-global config only.
    #[error(
        "repo-local override file {file:?} refused — the test-command override is read ONLY \
         from the daemon-owned user-global config (set HARNESS_TEST_COMMAND / \
         DaemonConfig.test_command); remove the repo-local file"
    )]
    RepoLocalOverride {
        /// The refused file (display path, forward slashes).
        file: String,
    },
    /// No known framework at the tested root.
    #[error(
        "unknown test framework at {searched:?} (no Cargo.toml) — day-one detection covers \
         cargo (cargo nextest / cargo test); to run another framework set the user-global \
         command override via HARNESS_TEST_COMMAND (DaemonConfig.test_command)"
    )]
    UnknownFramework {
        /// The tested root (display path, forward slashes).
        searched: String,
    },
    /// The run failed with no parseable per-test results (test binary
    /// crash or compile failure) — the exit code and stderr head carry
    /// the cause.
    #[error(
        "{command} failed with exit code {exit_code:?} and produced no parseable per-test \
         results — stderr: {stderr_head}"
    )]
    RunFailed {
        /// The command that ran.
        command: String,
        /// The child's exit code (`None` = abnormal termination).
        exit_code: Option<i32>,
        /// Bounded stderr head.
        stderr_head: String,
    },
    /// Report serialization failed (unreachable without a bug).
    #[error("internal error: {detail}")]
    Internal {
        /// The serializer failure.
        detail: String,
    },
    /// The spawn lane failed or timed out.
    #[error("{0}")]
    Run(#[from] RunError),
}
