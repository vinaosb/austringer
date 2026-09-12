//! Shared helpers for the PTY integration suites.

#![allow(dead_code, reason = "shared test helpers")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use harness_exec::runner::PtyRunner;
use harness_exec::spec::{ExecSpec, RunIn, ScreenSize};

pub fn spec(program: &str, args: &[&str]) -> ExecSpec {
    ExecSpec {
        program: program.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        cwd: None,
        env: Default::default(),
        timeout: Duration::from_secs(30),
        size: ScreenSize::default(),
        run_in: RunIn::Native,
    }
}

pub fn spec_with_timeout(program: &str, args: &[&str], timeout: Duration) -> ExecSpec {
    let mut command = spec(program, args);
    command.timeout = timeout;
    command
}

pub fn runner_with_temp_spill(dir: &tempfile::TempDir) -> PtyRunner {
    PtyRunner::with_spill_root(dir.path())
}

static SEQ: AtomicUsize = AtomicUsize::new(0);
pub fn unique_stem(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// Serializes live-ConPTY tests within this binary (todo 20 flake fix):
/// real terminals contend for conhost attention under parallel load, and
/// cargo's default thread-per-test parallelism multiplies that load.
/// External load (parallel builds) is absorbed by generous spec timeouts
/// instead — this gate only removes the suite's own self-inflicted
/// contention.
pub struct LiveGate(std::sync::MutexGuard<'static, ()>);

pub fn live_gate() -> LiveGate {
    static GATE: OnceLock<Mutex<()>> = OnceLock::new();
    LiveGate(
        GATE.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
}

/// Count live processes whose executable name equals `name`
/// (case-insensitive) — the orphan-enumeration oracle.
pub fn count_processes(system: &mut sysinfo::System, name: &str) -> usize {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .values()
        .filter(|process| process.name().to_string_lossy().eq_ignore_ascii_case(name))
        .count()
}

/// Poll until `predicate` holds or an 8 s deadline passes; returns the
/// last observation.
pub fn wait_until<T>(mut probe: impl FnMut() -> T, predicate: impl Fn(&T) -> bool) -> T {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let observed = probe();
        if predicate(&observed) || Instant::now() >= deadline {
            return observed;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Writes newline-terminated `line` text into a temp file until it
/// reaches `total_bytes`; returns its path.
pub fn big_text_file(dir: &std::path::Path, total_bytes: usize, line: &str) -> PathBuf {
    use std::io::Write;
    let path = dir.join(format!("{}.txt", unique_stem("big")));
    let mut file = std::fs::File::create(&path).expect("create big file");
    let line_bytes = format!("{line}\r\n").into_bytes();
    let mut written = 0usize;
    while written + line_bytes.len() <= total_bytes {
        file.write_all(&line_bytes).expect("write big file");
        written += line_bytes.len();
    }
    file.sync_all().expect("sync big file");
    path
}
