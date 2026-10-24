//! Kill-tree chaos tests (plan todo 19): timeout trees, orphan-free
//! enumeration proofs, cancellation, repeated interruptions.
//!
//! Orphan oracle: every tree test counts live processes by exe name AND
//! a unique marker in its child argv (`count_processes_marked`). Nextest
//! runs each test in its own process, so sibling chaos tests' transient
//! ping/tail trees run concurrently and a bare name count snapshots
//! their noise into the baseline (CI receipt 2026-10-24: baseline 1 ->
//! after 0 inverted the straggler assert on a shared windows runner).
//! The per-test marker makes each count hermetic and lets the asserts
//! demand a hard zero both before and after the run.

mod common;

#[cfg(windows)]
use std::thread;
use std::time::Duration;

use common::{count_processes_marked, runner_with_temp_spill, spec, spec_with_timeout, wait_until};
#[cfg(windows)]
use harness_exec::runner::CancelToken;
use harness_exec::spec::ExecStatus;
use sysinfo::System;

/// cmd spawns a `ping` grandchild (`start /b`) aimed at `target` and
/// then sleeps in its own `ping` to the same marker: the tree is
/// cmd -> { ping, ping }. Killing only cmd would orphan both pings; the
/// kill domain must erase the whole tree. Each caller passes a unique
/// loopback target so concurrent tests can never cross-count.
#[cfg(windows)]
fn tree_spec(timeout: Duration, target: &str) -> harness_exec::spec::ExecSpec {
    let command = format!("start /b ping -n 600 {target} > NUL & ping -n 600 {target} > NUL");
    spec_with_timeout("cmd", &["/c", &command], timeout)
}

#[cfg(windows)]
#[test]
fn timeout_kills_the_whole_tree_no_orphans() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();
    assert_eq!(
        count_processes_marked(&mut system, "PING.EXE", "127.0.0.2"),
        0,
        "marker pre-exists — leaked from a prior run"
    );

    let outcome = runner
        .run(&tree_spec(Duration::from_secs(1), "127.0.0.2"))
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::TimedOut);

    // Enumeration oracle: every ping from our tree must be gone.
    let after = wait_until(
        || count_processes_marked(&mut system, "PING.EXE", "127.0.0.2"),
        |count| *count == 0,
    );
    assert_eq!(after, 0, "kill tree must leave zero orphaned pings");
}

#[cfg(windows)]
#[test]
fn cancel_mid_command_kills_tree_with_typed_cancelled_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let cancel = CancelToken::new();
    let cancel_clone = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(600));
        cancel_clone.cancel();
    });
    let mut system = System::new();
    assert_eq!(
        count_processes_marked(&mut system, "PING.EXE", "127.0.0.3"),
        0,
        "marker pre-exists — leaked from a prior run"
    );

    let started = std::time::Instant::now();
    let outcome = runner
        .run_with_cancel(&tree_spec(Duration::from_secs(120), "127.0.0.3"), &cancel)
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::Cancelled);
    assert!(
        outcome.elapsed < Duration::from_secs(30),
        "cancel must beat the 120 s timeout"
    );
    let _ = started;
    let after = wait_until(
        || count_processes_marked(&mut system, "PING.EXE", "127.0.0.3"),
        |count| *count == 0,
    );
    assert_eq!(after, 0, "cancelled tree leaves zero orphans");
}

#[cfg(windows)]
#[test]
fn repeated_interruptions_kill_respawn_kill_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();
    assert_eq!(
        count_processes_marked(&mut system, "PING.EXE", "127.0.0.4"),
        0,
        "marker pre-exists — leaked from a prior run"
    );

    for round in 0..3 {
        let outcome = runner
            .run(&tree_spec(Duration::from_millis(500), "127.0.0.4"))
            .expect("run");
        assert_eq!(
            outcome.status,
            ExecStatus::TimedOut,
            "round {round} must time out"
        );
        let after = wait_until(
            || count_processes_marked(&mut system, "PING.EXE", "127.0.0.4"),
            |count| *count == 0,
        );
        assert_eq!(after, 0, "round {round} leaves zero orphans");
    }
}

#[cfg(not(windows))]
#[test]
fn unix_timeout_kills_process_group_no_orphans() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    // sh spawns a background tail (same process group) then tails
    // itself; a timeout must killpg the whole group. /dev/zero is this
    // test's hermetic marker (the straggler test owns /dev/null).
    let mut system = System::new();
    assert_eq!(
        count_processes_marked(&mut system, "tail", "/dev/zero"),
        0,
        "marker pre-exists — leaked from a prior run"
    );
    let outcome = runner
        .run(&spec_with_timeout(
            "sh",
            &["-c", "tail -f /dev/zero & tail -f /dev/zero"],
            Duration::from_secs(1),
        ))
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::TimedOut);
    let after = wait_until(
        || count_processes_marked(&mut system, "tail", "/dev/zero"),
        |count| *count == 0,
    );
    assert_eq!(after, 0, "killpg must leave zero orphaned tails");
}

#[test]
fn normal_completion_leaves_no_straggler_children() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    // The parent exits immediately while a grandchild keeps running.
    // UseShellExecute=false forces the CreateProcess path so the
    // grandchild inherits our job (shell-launched processes break away;
    // `cmd start /b` and Start-Process -RedirectStandardOutput both hang
    // their own parent — three Windows quirks verified on this machine).
    #[cfg(windows)]
    let (program, args, name, marker): (&str, &[&str], &str, &str) = (
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "$p = New-Object Diagnostics.ProcessStartInfo; $p.FileName = 'ping'; $p.Arguments = '-n 600 127.0.0.5'; $p.UseShellExecute = $false; [Diagnostics.Process]::Start($p) | Out-Null; 'spawned'",
        ],
        "PING.EXE",
        "127.0.0.5",
    );
    #[cfg(not(windows))]
    let (program, args, name, marker): (&str, &[&str], &str, &str) = (
        "sh",
        &["-c", "tail -f /dev/null & exit 0"],
        "tail",
        "/dev/null",
    );

    let mut system = System::new();
    assert_eq!(
        count_processes_marked(&mut system, name, marker),
        0,
        "marker pre-exists — leaked from a prior run"
    );
    let outcome = runner.run(&spec(program, args)).expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });

    let after = wait_until(
        || count_processes_marked(&mut system, name, marker),
        |count| *count == 0,
    );
    assert_eq!(after, 0, "completed command leaves zero stragglers");
}
