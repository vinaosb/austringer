//! Kill-tree chaos tests (plan todo 19): timeout trees, orphan-free
//! enumeration proofs, cancellation, repeated interruptions.

mod common;

#[cfg(windows)]
use std::thread;
use std::time::Duration;

use common::{count_processes, runner_with_temp_spill, spec, spec_with_timeout, wait_until};
#[cfg(windows)]
use harness_exec::runner::CancelToken;
use harness_exec::spec::ExecStatus;
use sysinfo::System;

/// cmd spawns a `ping` grandchild (`start /b`) and then sleeps in its own
/// `ping`: the tree is cmd -> { ping, ping }. Killing only cmd would
/// orphan both pings; the kill domain must erase the whole tree.
#[cfg(windows)]
const TREE_COMMAND: &str = "start /b ping -n 600 127.0.0.1 > NUL & ping -n 600 127.0.0.1 > NUL";

#[cfg(windows)]
fn tree_spec(timeout: Duration) -> harness_exec::spec::ExecSpec {
    spec_with_timeout("cmd", &["/c", TREE_COMMAND], timeout)
}

#[cfg(windows)]
#[test]
fn timeout_kills_the_whole_tree_no_orphans() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();

    let baseline = count_processes(&mut system, "PING.EXE");
    let outcome = runner.run(&tree_spec(Duration::from_secs(1))).expect("run");
    assert_eq!(outcome.status, ExecStatus::TimedOut);

    // Enumeration oracle: every ping from our tree must be gone.
    let after = wait_until(
        || count_processes(&mut system, "PING.EXE"),
        |count| *count <= baseline,
    );
    assert_eq!(
        after, baseline,
        "kill tree must leave zero orphaned pings (baseline {baseline}, saw {after})"
    );
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
    let baseline = count_processes(&mut system, "PING.EXE");

    let started = std::time::Instant::now();
    let outcome = runner
        .run_with_cancel(&tree_spec(Duration::from_secs(120)), &cancel)
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::Cancelled);
    assert!(
        outcome.elapsed < Duration::from_secs(30),
        "cancel must beat the 120 s timeout"
    );
    let _ = started;
    let after = wait_until(
        || count_processes(&mut system, "PING.EXE"),
        |count| *count <= baseline,
    );
    assert_eq!(after, baseline, "cancelled tree leaves zero orphans");
}

#[cfg(windows)]
#[test]
fn repeated_interruptions_kill_respawn_kill_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();
    let baseline = count_processes(&mut system, "PING.EXE");

    for round in 0..3 {
        let outcome = runner
            .run(&tree_spec(Duration::from_millis(500)))
            .expect("run");
        assert_eq!(
            outcome.status,
            ExecStatus::TimedOut,
            "round {round} must time out"
        );
        let after = wait_until(
            || count_processes(&mut system, "PING.EXE"),
            |count| *count <= baseline,
        );
        assert_eq!(after, baseline, "round {round} leaves zero orphans");
    }
}

#[cfg(not(windows))]
#[test]
fn unix_timeout_kills_process_group_no_orphans() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    // sh spawns a background sleeper (same process group) then sleeps
    // itself; a timeout must killpg the whole group.
    let outcome = runner
        .run(&spec_with_timeout(
            "sh",
            &["-c", "sleep 600 & sleep 600"],
            Duration::from_secs(1),
        ))
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::TimedOut);
    let mut system = System::new();
    let after = wait_until(
        || count_processes(&mut system, "sleep"),
        |count| *count == 0,
    );
    assert_eq!(after, 0, "killpg must leave zero orphaned sleepers");
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
    let (program, args): (&str, &[&str]) = (
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "$p = New-Object Diagnostics.ProcessStartInfo; $p.FileName = 'ping'; $p.Arguments = '-n 600 127.0.0.1'; $p.UseShellExecute = $false; [Diagnostics.Process]::Start($p) | Out-Null; 'spawned'",
        ],
    );
    #[cfg(not(windows))]
    let (program, args): (&str, &[&str]) = ("sh", &["-c", "sleep 600 & exit 0"]);
    #[cfg(windows)]
    let marker = "PING.EXE";
    #[cfg(not(windows))]
    let marker = "sleep";

    let mut system = System::new();
    let baseline = count_processes(&mut system, marker);
    let outcome = runner.run(&spec(program, args)).expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });

    let after = wait_until(
        || count_processes(&mut system, marker),
        |count| *count <= baseline,
    );
    assert_eq!(
        after, baseline,
        "completed command leaves zero stragglers (baseline {baseline})"
    );
}
