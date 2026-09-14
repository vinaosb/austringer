//! Process scenarios: CLI cold start (min-of-N spawn-to-exit), daemon RSS
//! at ready (stdout ready-line + targeted sysinfo sample), binary-size
//! budget, and the self-test breach probe. Skips are typed when their
//! input binaries were not provided.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::{ScenarioContext, ScenarioError, ScenarioOutput, ScenarioResult};
use crate::gate::Policy;
use crate::stats::min_of;

const SPAWN_TIMEOUT: Duration = Duration::from_secs(15);
const READY_TIMEOUT: Duration = Duration::from_secs(20);

fn wait_success(child: &mut Child, what: &str) -> Result<(), ScenarioError> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(ScenarioError::Failed(format!("{what} exited {status}")));
            }
            Ok(None) if started.elapsed() > SPAWN_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ScenarioError::Failed(format!("{what} timed out")));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(e) => return Err(ScenarioError::Failed(format!("{what} wait failed: {e}"))),
        }
    }
}

/// `<50 ms` cold start: cheapest real binary exit path, min-of-N.
pub fn cold_start(ctx: &ScenarioContext) -> ScenarioResult {
    let bin = ctx
        .cli_bin
        .clone()
        .ok_or_else(|| ScenarioError::Skipped("--cli-bin not provided".into()))?;
    let mut walls = Vec::new();
    for _ in 0..ctx.wall_runs.max(1) {
        let started = Instant::now();
        let mut child = Command::new(&bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| ScenarioError::Failed(format!("spawn {}: {e}", bin.display())))?;
        wait_success(&mut child, "cli --version")?;
        walls.push(started.elapsed());
    }
    let mut out = ScenarioOutput::new();
    out.push(
        "cold_start_ms",
        min_of(&walls)
            .map_err(|e| ScenarioError::Failed(e.to_string()))?
            .as_secs_f64()
            * 1000.0,
        "ms",
        Policy::Budget {
            budget: ctx.budgets.cold_start_ms as f64,
        },
    );
    Ok(out)
}

/// Kills the daemon on every exit path (no orphaned gate daemons).
struct DaemonGuard(Option<Child>);

impl DaemonGuard {
    fn take(&mut self) -> Option<&mut Child> {
        self.0.as_mut()
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Daemon RSS sampled at the ready line (todo-18 targeted-pid pattern).
pub fn daemon_rss(ctx: &ScenarioContext) -> ScenarioResult {
    let bin = ctx
        .daemon_bin
        .clone()
        .ok_or_else(|| ScenarioError::Skipped("--daemon-bin not provided".into()))?;
    let dir =
        tempfile::TempDir::new().map_err(|e| ScenarioError::Failed(format!("tempdir: {e}")))?;
    let ipc_name = format!("harness-bench-rss-{}", std::process::id());
    let child = Command::new(&bin)
        .arg("--data-dir")
        .arg(dir.path())
        .arg("--ipc-name")
        .arg(&ipc_name)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| ScenarioError::Failed(format!("spawn daemon: {e}")))?;
    let pid = child.id();
    let mut guard = DaemonGuard(Some(child));

    let started = Instant::now();
    let stdout = guard
        .take()
        .and_then(|c| c.stdout.take())
        .ok_or_else(|| ScenarioError::Failed("daemon stdout unavailable".into()))?;
    let mut reader = BufReader::new(stdout);
    let mut ready = false;
    loop {
        if ready {
            break;
        }
        if started.elapsed() > READY_TIMEOUT {
            return Err(ScenarioError::Failed("daemon never reported ready".into()));
        }
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| ScenarioError::Failed(format!("daemon stdout: {e}")))?;
        if read == 0 {
            return Err(ScenarioError::Failed(
                "daemon exited before ready line".into(),
            ));
        }
        ready = line.contains("listening on http://");
    }

    let mut system = sysinfo::System::new();
    let target = sysinfo::Pid::from_u32(pid);
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[target]), true);
    let rss_bytes = system
        .process(target)
        .map(|p| p.memory())
        .ok_or_else(|| ScenarioError::Failed("daemon RSS sample failed".into()))?;
    let rss_mb = rss_bytes as f64 / (1024.0 * 1024.0);

    let mut out = ScenarioOutput::new();
    out.push(
        "rss_mb",
        rss_mb,
        "MB",
        Policy::Budget {
            budget: ctx.budgets.rss_mb as f64,
        },
    );
    Ok(out)
}

/// Stripped release binary size budget (each named target).
pub fn binary_size(ctx: &ScenarioContext) -> ScenarioResult {
    if ctx.binaries.is_empty() {
        return Err(ScenarioError::Skipped(
            "no --binary targets provided".into(),
        ));
    }
    let mut out = ScenarioOutput::new();
    for (name, path) in &ctx.binaries {
        let bytes = std::fs::metadata(Path::new(path))
            .map_err(|e| ScenarioError::Failed(format!("binary {name} {}: {e}", path.display())))?
            .len();
        out.push(
            &format!("binary.{name}_mb"),
            bytes as f64 / (1024.0 * 1024.0),
            "MB",
            Policy::Budget {
                budget: ctx.budgets.binary_mb as f64,
            },
        );
    }
    Ok(out)
}

/// Gate self-test: MEASURED busy-well above the cold-start budget; the
/// evaluation MUST breach and the gate exit non-zero. End-to-end proof
/// that breaches are detected from real measurements, not constants.
pub fn self_test(ctx: &ScenarioContext) -> ScenarioResult {
    let target = Duration::from_millis(ctx.budgets.cold_start_ms.saturating_mul(3));
    let started = Instant::now();
    while started.elapsed() < target {
        std::hint::spin_loop();
    }
    let mut out = ScenarioOutput::new();
    out.push(
        "self_test.cold_start_ms",
        started.elapsed().as_secs_f64() * 1000.0,
        "ms",
        Policy::Budget {
            budget: ctx.budgets.cold_start_ms as f64,
        },
    );
    Ok(out)
}
