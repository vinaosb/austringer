//! M6 standing binary-size canary (plan M6: "the M27 <40 MB gate must
//! never be the first measurement").
//!
//! `#[ignore]`d by design - it runs a real `cargo build --release` of the
//! daemon (minutes, not hermetic-milliseconds), so it never runs in CI.
//! It is run ONCE PER HEAVY-DEP ADAPTER TODO (M12/M13/M14/M22/M28
//! minimum) and at M27, and its output line is copied verbatim into that
//! todo's evidence file.
//!
//! Run: `cargo nextest run -p austringer-media --run-ignored only --test size_canary`
//! (set `CARGO_TARGET_DIR` to reuse an existing release target dir).
//!
//! Record format (one line, machine-parseable, stable field order):
//!
//! ```text
//! SIZE_CANARY daemon=<MB> baseline=<MB> delta=<+/-MB(+/-pct)> budget=<MB> verdict=OK|OVER-BUDGET|REGRESSION
//! ```
//!
//! `BASELINE_MIB` is the last recorded size (this file's history IS the
//! ledger; the number must never move without an evidence line landing in
//! the same todo). A >10% growth over the baseline (the workspace
//! regression tolerance) or a breach of the 40 MB budget fails the test
//! naming the evidence duty.

use std::path::PathBuf;
use std::process::Command;

/// Daemon binary-size budget in MiB (austringer-bench budgets.json
/// `binary_mb`; the M27 gate re-verifies it with media features on).
pub const SIZE_BUDGET_MIB: f64 = 40.0;

/// Baseline recorded at M6 (2026-09-22, dev machine, evidence
/// task-M6-media-formats.txt): 6_261_248 bytes = 5.97 MiB, daemon
/// release build in the M9-in-flight worktree with the M6 executor
/// changes compiled in.
pub const BASELINE_MIB: f64 = 5.967;

/// Regression tolerance (workspace `regression_tolerance_pct`).
pub const REGRESSION_TOLERANCE_PCT: f64 = 10.0;

fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1048576.0
}

fn daemon_exe(target_dir: &std::path::Path) -> PathBuf {
    let name = if cfg!(windows) {
        "austringer-daemon.exe"
    } else {
        "austringer-daemon"
    };
    target_dir.join("release").join(name)
}

#[test]
#[ignore = "runs a real release build of the daemon; run at each heavy-dep adapter todo"]
fn binary_size_canary_records_baseline_and_delta() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));

    // Build (never enabled/downloaded/spawned by default applies to TOOLS,
    // not to the daemon's own compile; media features on the daemon arrive
    // with the M9/M32 wiring - until then this measures the pre-media
    // daemon, the baseline every heavy dep is delta'd against).
    let build = Command::new(env!("CARGO"))
        .args(["build", "--release", "-p", "austringer-daemon"])
        .current_dir(&workspace)
        .output()
        .expect("cargo build must be spawnable");
    assert!(
        build.status.success(),
        "daemon release build failed in {}:\nstdout: {}\nstderr: {}",
        workspace.display(),
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let exe = daemon_exe(&target_dir);
    let size = std::fs::metadata(&exe)
        .unwrap_or_else(|error| panic!("daemon binary missing at {}: {error}", exe.display()))
        .len();
    let current = mib(size);
    let delta_mb = current - BASELINE_MIB;
    let delta_pct = (current / BASELINE_MIB - 1.0) * 100.0;
    let over_budget = current > SIZE_BUDGET_MIB;
    let regressed = delta_pct > REGRESSION_TOLERANCE_PCT;
    let verdict = if over_budget {
        "OVER-BUDGET"
    } else if regressed {
        "REGRESSION"
    } else {
        "OK"
    };
    println!(
        "SIZE_CANARY daemon={current:.2} baseline={BASELINE_MIB:.2} delta={delta_mb:+.2}MiB({delta_pct:+.1}%) budget={SIZE_BUDGET_MIB:.2} verdict={verdict}"
    );
    assert!(
        !over_budget,
        "daemon {current:.2} MiB exceeds the {SIZE_BUDGET_MIB} MiB budget - record the delta in this todo's evidence and shed the weight before landing"
    );
    assert!(
        !regressed,
        "daemon {current:.2} MiB is >{REGRESSION_TOLERANCE_PCT}% over the {BASELINE_MIB:.2} MiB baseline - heavy-dep todo duty: update BASELINE_MIB here AND record the size+delta line in .omo/evidence/ (never silently)"
    );
}
