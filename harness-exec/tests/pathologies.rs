//! The three Windows ConPTY pathology fixtures (plan todo 20). Each test
//! reproduces the ORIGINAL failure shape. The old failure mode was an
//! unbounded hang, so "green" means the run completes far inside the
//! pathological bound.
//!
//! Load-tolerance policy (todo-20 follow-up): absolute wall-time bounds
//! went red under heavy parallel machine load (observed: DA1 whole-run
//! <5 s bound vs 6.5 s powershell spawn under load; headless 10-run
//! <30 s vs 49.5 s under load) even though the INTERNAL metrics prove the
//! pathologies are fixed (DA1-MS=7, DSR-MS=10 — ms-level). So:
//!   (a) the PRIMARY pass criteria are the internal observables — query
//!       response latencies (DA1-MS/DSR-MS), teardown-ordering
//!       (reader_joined), and orphan enumerations (PING.EXE counts);
//!   (b) where a wall-time bound remains useful it is either relaxed to
//!       3-5× the calm-measured value or made RELATIVE to a spawn baseline
//!       measured within the same run (bound = k × spawn-baseline), so it
//!       inflates with load instead of going red;
//!   (c) every fixture's pathology-coverage is intact — none deleted or
//!       skipped.
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

use common::{
    count_processes, parse_ms, runner_with_temp_spill, scaled_bound, spawn_baseline, spec,
    spec_with_timeout, wait_until,
};
use harness_exec::runner::CancelToken;
use harness_exec::spec::ExecStatus;
use sysinfo::System;

// Calm-machine floor for a single headless run. The old pathology was an
// unbounded hang; a healthy run is ~tens of ms. Relaxed to 5× the calm
// 8 s value so a loaded machine (spawn inflation) stays green while a
// genuine hang still trips it.
const PATHOLOGY_BOUND: Duration = Duration::from_secs(40);
// Calm-machine floor for the DA1/DSR whole-run wall time. The pathology
// was a ~60 s block; healthy is single-digit ms. Relaxed to 5× the calm
// 5 s value so a slow powershell spawn under load stays green.
const DA1_BOUND: Duration = Duration::from_secs(25);

// ---------- (a) headless completion: no hang, no leaked reader ----------

#[test]
fn headless_completion_returns_promptly_with_reader_joined() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    // Load-relative bound: 5× the measured spawn cost of one cmd, floored
    // at the calm 40 s. Under load the spawn baseline inflates (observed
    // 49.5 s for 10 runs), so the bound follows it instead of going red.
    let spawn = spawn_baseline("cmd", &["/c", "echo", "baseline"]);
    let per_round = scaled_bound(spawn, 5, PATHOLOGY_BOUND);
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
            elapsed < per_round,
            "round {round} hung: {elapsed:?} (old pathology: unbounded)"
        );
        assert!(
            outcome.reader_joined,
            "round {round}: blocked reader must be unblocked and joined, not leaked"
        );
    }
    // 10 runs must stay far from hang territory. The per-round bound
    // already catches a hang; this total is a loose collective sanity
    // check, so it is exactly 10× the per-round bound (consistent with 10
    // rounds) rather than a tight absolute that flakes under load.
    let total_bound = per_round.saturating_mul(10);
    assert!(
        total < total_bound,
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
    // Load-relative bound: 5× the cmd spawn baseline, floored at 40 s.
    let spawn = spawn_baseline("cmd", &["/c", "echo", "baseline"]);
    assert!(
        outcome.elapsed < scaled_bound(spawn, 5, PATHOLOGY_BOUND),
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
    // Load-relative bound: 5× the cmd spawn baseline, floored at 40 s.
    let spawn = spawn_baseline("cmd", &["/c", "echo", "baseline"]);
    let bound = scaled_bound(spawn, 5, PATHOLOGY_BOUND);
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
            outcome.elapsed < bound,
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
    // Load-relative bound: 5× the cmd spawn baseline, floored at 15 s.
    let spawn = spawn_baseline("cmd", &["/c", "echo", "baseline"]);
    let bound = scaled_bound(spawn, 5, Duration::from_secs(15));
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
                assert!(outcome.elapsed < bound);
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
//
// Load-tolerance: the PRIMARY pass criterion is the child-reported
// DA1-MS/DSR-MS latency (ms-level, load-insensitive). The whole-run wall
// bound is relaxed to 5× the calm 5 s value (25 s) so a slow powershell
// spawn under load (observed 6.5 s) stays green while a genuine ~60 s
// hang still trips it.

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
    // PRIMARY: the child must have READ both replies, and the measured
    // query-response latency must be ms-level (the pathology was ~60 s).
    assert!(
        outcome.model_text.contains("DA1-SEEN=") && outcome.model_text.contains("DA1-MS="),
        "child must have READ replies: {}",
        outcome.model_text
    );
    let da1_ms =
        parse_ms(&outcome.model_text, "DA1-MS").expect("DA1-MS must be present and numeric");
    assert!(
        da1_ms < 5_000,
        "DA1 reply latency must be ms-level, not the ~60 s timeout: {da1_ms} ms"
    );
    // SECONDARY: whole-run wall time (includes spawn + teardown) stays
    // far below the pathological ~60 s block. Relaxed to 5× calm so a
    // slow powershell spawn under load stays green.
    assert!(
        outcome.elapsed < DA1_BOUND,
        "DA1 hang shape: {:?} — {}",
        outcome.elapsed,
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
    // PRIMARY: the child must have READ the cursor report, and the
    // measured query-response latency must be ms-level.
    assert!(
        outcome.model_text.contains("DSR-SEEN="),
        "child read a cursor report: {}",
        outcome.model_text
    );
    let dsr_ms =
        parse_ms(&outcome.model_text, "DSR-MS").expect("DSR-MS must be present and numeric");
    assert!(
        dsr_ms < 5_000,
        "DSR reply latency must be ms-level, not the ~60 s timeout: {dsr_ms} ms"
    );
    // SECONDARY: whole-run wall time stays far below the pathological
    // ~60 s block. Relaxed to 5× calm so a slow powershell spawn under
    // load stays green.
    assert!(
        outcome.elapsed < DA1_BOUND,
        "DSR hang shape: {:?} — {}",
        outcome.elapsed,
        outcome.model_text
    );
}
