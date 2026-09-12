//! QA evidence channels (plan todo 19). Ignored by default — run with
//! `cargo test -p harness-exec --test pty_qa -- --ignored --nocapture`
//! and paste the output into the evidence file.

mod common;

use std::time::{Duration, Instant};

use common::{big_text_file, count_processes, runner_with_temp_spill, spec, spec_with_timeout};
use harness_core::tools::SpillStore;
use harness_exec::spec::ExecStatus;
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
