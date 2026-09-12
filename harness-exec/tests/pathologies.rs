//! The three Windows ConPTY pathology fixtures (plan todo 20). Each test
//! reproduces the ORIGINAL failure shape with a bounded wall-time assert:
//! the old failure mode was an unbounded hang, so "green" means the run
//! completes far inside the pathological bound.
//!
//! (a) headless wait()/read hang: command completes but the PTY reader
//!     stays blocked — the run must return promptly AND the reader thread
//!     must be joined (a detached reader leaks a thread + pipe handle per
//!     run, which is handle exhaustion under load).
//! (b) Arc shutdown assertion / teardown ordering: dropping the run
//!     mid-active-command must tear down in Job-Object-kill-first order
//!     with zero orphans, bounded, repeatedly.
//! (c) DA1 `ESC[c` 60 s hang: a child that queries primary device
//!     attributes and blocks for the reply must get it in milliseconds,
//!     not the ~60 s conhost timeout.

mod common;

use std::thread;
use std::time::{Duration, Instant};

use common::{count_processes, runner_with_temp_spill, spec, spec_with_timeout, wait_until};
use harness_exec::runner::CancelToken;
use harness_exec::spec::ExecStatus;
use sysinfo::System;

const PATHOLOGY_BOUND: Duration = Duration::from_secs(8);
const DA1_BOUND: Duration = Duration::from_secs(5);

// ---------- (a) headless completion: no hang, no leaked reader ----------

#[test]
fn headless_completion_returns_promptly_with_reader_joined() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut total = Duration::ZERO;
    for round in 0..10 {
        let started = Instant::now();
        let outcome = runner
            .run(&spec("cmd", &["/c", "echo", "headless-20"]))
            .expect("run");
        let elapsed = started.elapsed();
        total += elapsed;
        assert_eq!(
            outcome.status,
            ExecStatus::Completed { exit_code: 0 },
            "round {round}"
        );
        assert!(
            elapsed < PATHOLOGY_BOUND,
            "round {round} hung: {elapsed:?} (old pathology: unbounded)"
        );
        assert!(
            outcome.reader_joined,
            "round {round}: blocked reader must be unblocked and joined, not leaked"
        );
    }
    assert!(
        total < Duration::from_secs(30),
        "10 quick headless runs must stay far from hang territory: {total:?}"
    );
}

#[cfg(windows)]
#[test]
fn headless_completion_with_a_pipe_holding_grandchild_still_joins() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();
    let baseline = count_processes(&mut system, "PING.EXE");
    // The parent exits immediately; the grandchild keeps the ConPTY pipe
    // busy — the exact shape that used to leave the reader blocked
    // forever after a completed run.
    let outcome = runner
        .run(&spec(
            "cmd",
            &["/c", "start /b ping -n 120 127.0.0.1 > NUL & exit 0"],
        ))
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });
    assert!(
        outcome.elapsed < PATHOLOGY_BOUND,
        "run hung: {:?}",
        outcome.elapsed
    );
    assert!(
        outcome.reader_joined,
        "grandchild-held pipe must not leak the reader"
    );
    let after = wait_until(
        || count_processes(&mut system, "PING.EXE"),
        |count| *count <= baseline,
    );
    assert_eq!(after, baseline, "straggler grandchild reaped");
}

// ---------- (b) drop mid-active-run: ordered teardown, no assertion ----------

#[cfg(windows)]
#[test]
fn teardown_mid_active_run_is_bounded_ordered_and_repeatable() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let mut system = System::new();
    let baseline = count_processes(&mut system, "PING.EXE");
    for round in 0..10 {
        // Pre-cancelled token: the teardown path runs while the child is
        // fully active — job kill must precede ConPTY master teardown.
        let cancel = CancelToken::new();
        cancel.cancel();
        let outcome = runner
            .run_with_cancel(
                &spec_with_timeout(
                    "cmd",
                    &[
                        "/c",
                        "start /b ping -n 600 127.0.0.1 > NUL & ping -n 600 127.0.0.1 > NUL",
                    ],
                    Duration::from_secs(120),
                ),
                &cancel,
            )
            .expect("run");
        assert_eq!(outcome.status, ExecStatus::Cancelled, "round {round}");
        assert!(
            outcome.elapsed < PATHOLOGY_BOUND,
            "round {round} teardown hung: {:?}",
            outcome.elapsed
        );
        assert!(outcome.reader_joined, "round {round} reader must join");
        let after = wait_until(
            || count_processes(&mut system, "PING.EXE"),
            |count| *count <= baseline,
        );
        assert_eq!(after, baseline, "round {round} leaves zero orphans");
    }
}

#[cfg(windows)]
#[test]
fn concurrent_active_runs_tearing_down_do_not_deadlock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let handles: Vec<_> = (0..4)
        .map(|index| {
            let runner = runner_with_temp_spill(&dir);
            thread::spawn(move || {
                let cancel = CancelToken::new();
                let fire = cancel.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(300 + index * 100));
                    fire.cancel();
                });
                let outcome = runner
                    .run_with_cancel(
                        &spec_with_timeout(
                            "cmd",
                            &["/c", "start /b ping -n 600 127.0.0.1 > NUL & ping -n 600 127.0.0.1 > NUL"],
                            Duration::from_secs(90),
                        ),
                        &cancel,
                    )
                    .expect("run");
                assert_eq!(outcome.status, ExecStatus::Cancelled);
                assert!(outcome.elapsed < Duration::from_secs(15));
                assert!(outcome.reader_joined);
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("no panic, no deadlock");
    }
}

// ---------- (c) DA1 ESC[c: answered far below the 60 s timeout ----------
//
// Measured on this machine (todo 20): the 60 s DA1 hang has TWO flows.
// (A) conhost→terminal queries arrive on the master read side and block
//     conhost until answered — T19's reactive responder (src/query.rs)
//     answers them in pump latency, which is why anything runs at all.
// (B) child→conhost queries (a child writing ESC[c itself) are answered
//     BY conhost in ~0 ms — the reply is injected into the console input
//     buffer and echoed to the screen (observed as caret notation in the
//     screen model during fixture development), never reaching us.
// Proactive pre-injection at spawn was therefore NOT added: flow A is
// already answered reactively in milliseconds, and pre-injecting for
// flow B would feed stray input to children that never query. The
// fixtures pin both halves: a querying child completes bounded (B), and
// the responder's own query-matching is unit-pinned in src/query.rs (A).

#[cfg(windows)]
#[test]
fn da1_query_child_gets_a_reply_in_milliseconds_not_60_seconds() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    // The child writes DA1 + DSR queries, then reads the console input
    // buffer the way VT-aware apps do (ReadKey): the fixture fails if
    // replies take the ~60 s conhost timeout or never arrive.
    let script = concat!(
        "$e=[char]27;",
        "[Console]::Write(\"$e[c$e[6n\");",
        "[Console]::Out.Flush();",
        "$sw=[Diagnostics.Stopwatch]::StartNew();",
        "$first=[Console]::ReadKey($true);",
        "$second=[Console]::ReadKey($true);",
        "$sw.Stop();",
        "[Console]::WriteLine(\"DA1-SEEN=$([int]$first.KeyChar):$([int]$second.KeyChar) DA1-MS=$($sw.ElapsedMilliseconds)\")",
    );
    let started = Instant::now();
    let outcome = runner
        .run(&spec("powershell", &["-NoProfile", "-Command", script]))
        .expect("run");
    assert_eq!(
        outcome.status,
        ExecStatus::Completed { exit_code: 0 },
        "model text: {}",
        outcome.model_text
    );
    // Bounded wall-time: the pathology was a ~60 s block per unanswered
    // query; healthy is single-digit milliseconds.
    assert!(
        outcome.elapsed < DA1_BOUND,
        "DA1 hang shape: {:?} — {}",
        outcome.elapsed,
        outcome.model_text
    );
    assert!(
        outcome.model_text.contains("DA1-SEEN=") && outcome.model_text.contains("DA1-MS="),
        "child must have READ replies: {}",
        outcome.model_text
    );
    assert!(
        started.elapsed() < DA1_BOUND + Duration::from_secs(1),
        "wall time includes teardown: {:?}",
        started.elapsed()
    );
}

#[cfg(windows)]
#[test]
fn dsr_cursor_query_also_answers_below_the_timeout() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let script = concat!(
        "$e=[char]27;",
        "[Console]::Write(\"$e[6n\");",
        "[Console]::Out.Flush();",
        "$sw=[Diagnostics.Stopwatch]::StartNew();",
        "$key=[Console]::ReadKey($true);",
        "$sw.Stop();",
        "[Console]::WriteLine(\"DSR-SEEN=$([int]$key.KeyChar) DSR-MS=$($sw.ElapsedMilliseconds)\")",
    );
    let outcome = runner
        .run(&spec("powershell", &["-NoProfile", "-Command", script]))
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });
    assert!(
        outcome.elapsed < DA1_BOUND,
        "DSR hang shape: {:?} — {}",
        outcome.elapsed,
        outcome.model_text
    );
    assert!(
        outcome.model_text.contains("DSR-SEEN="),
        "child read a cursor report: {}",
        outcome.model_text
    );
}
