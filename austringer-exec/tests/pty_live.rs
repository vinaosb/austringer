//! Live PTY pipeline tests (plan todo 19): happy paths, exit codes,
//! bounded output, spill, stale-state immunity, malformed input.

// The fixtures drive cmd/powershell through a real ConPTY; unix live-PTY
// coverage lives in pty_chaos's explicit sh/sleep variants.
#![cfg(windows)]

mod common;

use std::time::Duration;

use austringer_core::tools::SpillStore;
use austringer_exec::spec::{ExecError, ExecStatus};
use common::{runner_with_temp_spill, spec, spec_with_timeout};

#[test]
fn echo_through_pty_yields_bounded_windowed_text() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let outcome = runner
        .run(&spec("cmd", &["/c", "echo hello-pty-19"]))
        .expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });
    assert!(
        outcome.model_text.contains("hello-pty-19"),
        "windowed content must carry the echo: {:?}",
        outcome.model_text
    );
    assert!(outcome.model_text.len() < 16 * 1024);
    assert!(outcome.spill.is_none(), "tiny output never spills");
}

#[cfg(windows)]
#[test]
fn cargo_version_style_command_renders_cleanly() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let outcome = runner.run(&spec("cargo", &["--version"])).expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });
    assert!(
        outcome.model_text.contains("cargo"),
        "content assertion, not boolean: {:?}",
        outcome.model_text
    );
    // ANSI/SGR must not survive: no CSI introducers in model text.
    assert!(!outcome.model_text.contains('\x1b'));
}

#[test]
fn nonzero_exit_code_surfaces_typed() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let outcome = runner.run(&spec("cmd", &["/c", "exit 42"])).expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 42 });
}

#[test]
fn missing_program_is_a_typed_spawn_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let error = runner
        .run(&spec("definitely-not-a-real-program-xyz-19", &[]))
        .expect_err("spawn must fail");
    assert!(matches!(error, ExecError::Spawn { .. }));
}

#[test]
fn timeout_validation_rejects_zero_and_over_ceiling() {
    let runner = austringer_exec::PtyRunner::new();
    let zero = spec_with_timeout("cmd", &["/c", "echo hi"], Duration::from_secs(0));
    assert!(matches!(
        runner.run(&zero),
        Err(ExecError::InvalidTimeout { .. })
    ));
    let absurd = spec_with_timeout(
        "cmd",
        &["/c", "echo hi"],
        austringer_exec::spec::MAX_COMMAND_TIMEOUT + Duration::from_secs(1),
    );
    assert!(matches!(
        runner.run(&absurd),
        Err(ExecError::InvalidTimeout { .. })
    ));
}

#[test]
fn oversized_output_spills_with_recoverable_pointer() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let big = common::big_text_file(dir.path(), 4 * 1024 * 1024, "SPAM-LINE-0123456789");
    let runner = runner_with_temp_spill(&dir);
    // Todo-20 flake fix: 4 MB through a real ConPTY took >30 s under
    // parallel-build load bursts (the verifier's TimedOut flake); the
    // wall-time budget now absorbs external load, while live_gate()
    // absorbs this suite's own parallelism.
    let mut command = spec("cmd", &["/c", "type", &big.to_string_lossy()]);
    command.timeout = Duration::from_secs(240);
    let outcome = runner.run(&command).expect("run");
    assert_eq!(outcome.status, ExecStatus::Completed { exit_code: 0 });

    // The window law bounds the model text with an omission marker.
    assert!(outcome.model_text.len() < 16 * 1024, "bounded model text");
    assert!(
        outcome.model_text.contains("lines omitted"),
        "marker present: {}",
        outcome.model_text.lines().count()
    );
    assert!(outcome.total_lines > 75, "large transcript recorded");

    // The raw stream spilled and is recoverable by pointer.
    let spill = outcome.spill.as_ref().expect("spilled");
    assert!(
        spill.raw_bytes > 1024 * 1024,
        "raw bytes counted: {}",
        spill.raw_bytes
    );
    let store = SpillStore::open(dir.path()).expect("store");
    let recovered = store.read(&spill.hash).expect("read").expect("present");
    assert_eq!(recovered.len() as u64, spill.raw_bytes);
    assert!(
        outcome.model_text.contains("SPAM-LINE"),
        "first window visible"
    );
}

#[test]
fn pty_reuse_starts_a_fresh_screen_per_command() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let first = runner
        .run(&spec("cmd", &["/c", "echo FIRST-MARKER-19"]))
        .expect("first");
    assert!(first.model_text.contains("FIRST-MARKER-19"));

    let second = runner
        .run(&spec("cmd", &["/c", "echo SECOND-MARKER-19"]))
        .expect("second");
    assert!(second.model_text.contains("SECOND-MARKER-19"));
    assert!(
        !second.model_text.contains("FIRST-MARKER-19"),
        "stale state must not leak across runs: {:?}",
        second.model_text
    );
}

#[cfg(windows)]
#[test]
fn binary_garbage_through_the_real_pty_never_panics() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let script = "$b = [byte[]](0..255) * 8; $o = [Console]::OpenStandardOutput(); $o.Write($b, 0, $b.Length); $o.Flush()";
    let outcome = runner
        .run(&spec("powershell", &["-NoProfile", "-Command", script]))
        .expect("run");
    assert!(outcome.model_text.len() < 16 * 1024);
    assert!(!outcome.model_text.contains('\x1b'));
}

#[cfg(windows)]
#[test]
fn tui_redraw_live_renders_final_frame() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let script = "$e = [char]27; [Console]::Write(\"$e[2J$e[HFRAME-ONE-19$e[3;5HMENU$e[2J$e[HFINAL-FRAME-19\")";
    let outcome = runner
        .run(&spec("powershell", &["-NoProfile", "-Command", script]))
        .expect("run");
    assert!(outcome.model_text.contains("FINAL-FRAME-19"));
    assert!(
        !outcome.model_text.contains("FRAME-ONE-19"),
        "cleared frames must not survive: {:?}",
        outcome.model_text
    );
}

#[cfg(windows)]
#[test]
fn live_progress_bar_yields_one_final_line() {
    let _gate = common::live_gate();
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = runner_with_temp_spill(&dir);
    let script = "1..100 | ForEach-Object { [Console]::Write(\"`rstep {0,3}%\" -f $_); Start-Sleep -Milliseconds 2 }; [Console]::Write(\"`rdone-live-19    \"); [Console]::WriteLine(\"\")";
    let outcome = runner
        .run(&spec("powershell", &["-NoProfile", "-Command", script]))
        .expect("run");
    assert!(
        outcome.model_text.contains("done-live-19"),
        "final line survives: {:?}",
        outcome.model_text
    );
    assert!(
        !outcome.model_text.contains("step"),
        "no intermediate progress spam: {:?}",
        outcome.model_text
    );
}
