//! QA evidence channels (plan todos 19 + 20). Ignored by default — run
//! with `cargo test -p harness-exec --test pty_qa -- --ignored --nocapture`
//! and paste the output into the evidence file.

mod common;

use std::time::{Duration, Instant};

use common::{big_text_file, runner_with_temp_spill, spec, spec_with_timeout};
use harness_core::tools::SpillStore;
use harness_exec::spec::ExecStatus;
// PING.EXE enumeration feeds only the windows QA fixtures.
#[cfg(windows)]
use common::count_processes;
#[cfg(windows)]
use sysinfo::System;

#[test]
#[ignore = "qa evidence channel"]
fn qa_happy_cargo_version_through_pty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let outcome = runner.run(&spec("cargo", &["--version"])).expect("run");
    println!("== QA happy: cargo --version through the PTY ==");
    println!("status: {:?}", outcome.status);
    println!("elapsed: {:?}", outcome.elapsed);
    println!("model_text ({:>5} bytes):", outcome.model_text.len());
    for line in outcome.model_text.lines() {
        println!("  | {line}");
    }
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });
}

#[cfg(windows)]
#[test]
#[ignore = "qa evidence channel"]
fn qa_progress_bar_collapses() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let frames = 150u32;
    let script = format!(
        "1..{frames} | ForEach-Object {{ [Console]::Write(\"`rloading {{0,3}}%\" -f $_); Start-Sleep -Milliseconds 3 }}; [Console]::Write(\"`rqa-complete-19    \"); [Console]::WriteLine(\"\")"
    );
    println!("== QA progress-bar: {frames} carriage-return frames ==");
    println!("-- BEFORE screen-delta (raw pump would emit, per frame):");
    println!("   `rloading   1%`rloading   2%`rloading   3%`r...`rloading 100%`");
    let outcome = runner
        .run(&spec("powershell", &["-NoProfile", "-Command", &script]))
        .expect("run");
    println!(
        "-- AFTER screen-delta (model-facing, {} bytes):",
        outcome.model_text.len()
    );
    for line in outcome.model_text.lines() {
        println!("  | {line}");
    }
    assert!(outcome.model_text.contains("qa-complete-19"));
    assert!(!outcome.model_text.contains("loading"));
}

#[cfg(windows)]
#[test]
#[ignore = "qa evidence channel"]
fn qa_timeout_tree_enumeration() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();
    let baseline = count_processes(&mut system, "PING.EXE");
    println!("== QA timeout kill-tree: enumeration before/after ==");
    println!("-- BEFORE: PING.EXE count = {baseline} (machine baseline)");

    let command = "start /b ping -n 600 127.0.0.1 > NUL & ping -n 600 127.0.0.1 > NUL";
    let started = Instant::now();
    let outcome = runner
        .run(&spec_with_timeout(
            "cmd",
            &["/c", command],
            Duration::from_secs(1),
        ))
        .expect("run");
    let after = common::wait_until(
        || count_processes(&mut system, "PING.EXE"),
        |count| *count <= baseline,
    );
    println!(
        "-- AFTER: status={:?} elapsed={:?} PING.EXE count = {} (baseline {baseline})",
        outcome.status,
        started.elapsed(),
        after
    );
    assert_eq!(outcome.status, ExecStatus::TimedOut);
    assert_eq!(after, baseline, "zero orphans by enumeration");
}

#[test]
#[ignore = "qa evidence channel: ~100 MB through a real ConPTY"]
fn qa_100mb_output_spills_bounded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let total = 100 * 1024 * 1024usize;
    let big = big_text_file(
        dir.path(),
        total,
        "QALINE-0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789",
    );
    println!(
        "== QA 100MB: generator file {} bytes ==",
        big.metadata().expect("meta").len()
    );

    let runner = runner_with_temp_spill(&dir);
    let started = Instant::now();
    let outcome = runner
        .run(&spec_with_timeout(
            "cmd",
            &["/c", "type", &big.to_string_lossy()],
            Duration::from_secs(600),
        ))
        .expect("run");
    let wall = started.elapsed();
    println!("-- status={:?} wall={wall:?}", outcome.status);
    println!(
        "-- model_text: {} bytes / {} windowed lines (budget 16 KiB / 75 lines)",
        outcome.model_text.len(),
        outcome.model_text.lines().count()
    );
    println!(
        "-- total transcript lines recorded: {}",
        outcome.total_lines
    );
    let spill = outcome.spill.as_ref().expect("spilled");
    println!(
        "-- spill pointer: {} ({} raw bytes)",
        spill.hash.to_ref(),
        spill.raw_bytes
    );
    let store = SpillStore::open(dir.path()).expect("store");
    let recovered = store.read(&spill.hash).expect("read").expect("present");
    println!("-- spill recovered: {} bytes", recovered.len());

    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });
    assert!(outcome.model_text.len() < 16 * 1024);
    assert!(outcome.model_text.contains("lines omitted"));
    assert!(outcome.total_lines > 75);
    assert_eq!(recovered.len() as u64, spill.raw_bytes);
    assert!(
        spill.raw_bytes > 90 * 1024 * 1024,
        "nearly all raw bytes captured"
    );
}

#[test]
#[ignore = "qa evidence channel: todo 20 headless pathology timings"]
fn qa_headless_pathology_timings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    println!("== QA todo-20 (a) headless completion: 10 runs, timings + reader join ==");
    let mut total = Duration::ZERO;
    for round in 1..=10 {
        let outcome = runner
            .run(&spec("cmd", &["/c", "echo", "qa-headless-20"]))
            .expect("run");
        total += outcome.elapsed;
        println!(
            "  run {round:2}: {:>8.2?}  status={:?}  reader_joined={}",
            outcome.elapsed, outcome.status, outcome.reader_joined
        );
        assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });
        assert!(outcome.reader_joined);
    }
    println!("  TOTAL: {total:?} (pathology bound: every run < 8 s, no hang)");
}

#[cfg(windows)]
#[test]
#[ignore = "qa evidence channel: todo 20 teardown enumeration under new drop path"]
fn qa_teardown_enumeration_under_new_drop_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();
    let baseline = count_processes(&mut system, "PING.EXE");
    println!("== QA todo-20 (b) mid-active-run teardown: enumeration under new drop path ==");
    println!("-- BEFORE: PING.EXE count = {baseline} (machine baseline)");
    let command = "start /b ping -n 600 127.0.0.1 > NUL & ping -n 600 127.0.0.1 > NUL";
    let cancel = harness_exec::runner::CancelToken::new();
    cancel.cancel();
    let started = Instant::now();
    let outcome = runner
        .run_with_cancel(
            &spec_with_timeout("cmd", &["/c", command], Duration::from_secs(120)),
            &cancel,
        )
        .expect("run");
    let after = common::wait_until(
        || count_processes(&mut system, "PING.EXE"),
        |count| *count <= baseline,
    );
    println!(
        "-- AFTER: status={:?} elapsed={:?} reader_joined={} PING.EXE count={after} (baseline {baseline})",
        outcome.status,
        started.elapsed(),
        outcome.reader_joined
    );
    assert_eq!(outcome.status, ExecStatus::Cancelled);
    assert_eq!(after, baseline, "runaway children die with the parent");
}

#[test]
#[ignore = "qa evidence channel: todo 20 WSL typed statuses"]
fn qa_wsl_typed_statuses() {
    println!("== QA todo-20 WSL bridge: typed statuses ==");
    let real = harness_exec::wsl::detect_wsl();
    println!("-- this machine: {real:?}");
    struct NoWsl;
    impl harness_exec::wsl::WslProbe for NoWsl {
        fn find_wsl_exe(&self) -> Option<std::path::PathBuf> {
            None
        }
        fn run_wsl(&self, _args: &[&str]) -> Option<(i32, harness_exec::wsl::WslProbeOutput)> {
            None
        }
    }
    let unavailable = harness_exec::wsl::detect_wsl_with(&NoWsl);
    println!("-- no-WSL machine: {unavailable:?}");
    let error = harness_exec::spec::ExecError::from(unavailable.clone());
    println!("-- typed error surfaced: {error}");
    assert_eq!(unavailable, harness_exec::wsl::WslStatus::NoWsl);
    if let harness_exec::wsl::WslStatus::Available { version, .. } = &real {
        println!("-- routing available, default version {version:?} (bridge target = Wsl2)");
    }
}
