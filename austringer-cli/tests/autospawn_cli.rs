//! Auto-spawn end to end: `austringer capture` with NO daemon running
//! spawns the real `austringer-daemon` binary (located via
//! AUSTRINGER_DAEMON_BIN), waits for the readiness files, connects,
//! and prints `queued` — proving the whole "just starts" path with
//! real processes. The spawned daemon is killed by pid at the end;
//! nothing leaks.

use assert_cmd::prelude::*;
use std::path::PathBuf;
use std::process::Command;

fn daemon_exe() -> PathBuf {
    let mut dir = std::env::current_exe()
        .unwrap_or_else(|e| panic!("current_exe: {e}"))
        .parent()
        .unwrap_or_else(|| panic!("no parent"))
        .to_path_buf();
    if dir.file_name().is_some_and(|name| name == "deps") {
        dir.pop();
    }
    let name = if cfg!(windows) {
        "austringer-daemon.exe"
    } else {
        "austringer-daemon"
    };
    let exe = dir.join(name);
    assert!(
        exe.is_file(),
        "daemon binary not found at {}",
        exe.display()
    );
    exe
}

fn kill_daemon(pid: u32) {
    let status = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .status()
    } else {
        Command::new("kill").args(["-9", &pid.to_string()]).status()
    };
    let _ = status.map(|status| status.code());
}

/// Hard backstop: this test spawns processes that spawn processes; if
/// any of them wedges, abort instead of hanging the runner. nextest
/// runs one test per process, so when the test finishes the process
/// exit kills the sleeping thread — no disarm needed.
fn arm_watchdog(seconds: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        eprintln!("autospawn e2e: wedged past {seconds}s; aborting");
        std::process::exit(101);
    });
}

fn pid_from_notice(stderr: &str) -> u32 {
    stderr
        .lines()
        .find_map(|line| line.split_once("pid "))
        .and_then(|(_, rest)| rest.split([')', ' ']).next())
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("no spawn notice with pid in: {stderr}"))
}

#[test]
fn capture_with_no_daemon_spawns_one_and_queues() {
    arm_watchdog(90);
    let data = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let ipc = format!("autospawn-test-{}", std::process::id());
    // The CLI's stderr must be a FILE, not a pipe: the detached daemon
    // would hold the pipe's write end and this test would block on EOF
    // until the daemon dies.
    let stderr_log = data.path().join("cli-stderr.log");
    let stderr_file = std::fs::File::create(&stderr_log).unwrap_or_else(|e| panic!("stderr: {e}"));
    let output = Command::cargo_bin("austringer")
        .unwrap_or_else(|e| panic!("cargo_bin: {e}"))
        // Global flags BEFORE the subcommand: `capture`'s prompt is a
        // trailing var arg, so anything after it joins the prompt text.
        .args([
            "--data-dir",
            data.path().to_str().unwrap_or_default(),
            "--ipc-name",
            &ipc,
            "capture",
            "hi",
        ])
        .env("AUSTRINGER_DAEMON_BIN", daemon_exe())
        .env_remove("AUSTRINGER_PROVIDER")
        .env_remove("AUSTRINGER_MODEL")
        .env_remove("AUSTRINGER_PROVIDER_KEY_ENV")
        .env_remove("AUSTRINGER_PROVIDER_BASE_URL")
        .stderr(std::process::Stdio::from(stderr_file))
        .output()
        .unwrap_or_else(|e| panic!("run cli: {e}"));
    let stderr =
        std::fs::read_to_string(&stderr_log).unwrap_or_else(|e| panic!("read stderr log: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    kill_daemon(pid_from_notice(&stderr));
    assert!(
        stderr.contains("spawned austringer-daemon (pid"),
        "missing spawn notice; stderr: {stderr}"
    );
    assert!(
        stdout.contains("queued"),
        "capture should submit and report queued; stdout: {stdout} stderr: {stderr}"
    );
    let token = std::fs::read_to_string(data.path().join("daemon-token"))
        .unwrap_or_else(|e| panic!("token file: {e}"));
    assert!(!token.trim().is_empty());
    let url = std::fs::read_to_string(data.path().join("daemon-url"))
        .unwrap_or_else(|e| panic!("url file: {e}"));
    assert!(
        url.starts_with("http://127.0.0.1:"),
        "unexpected url file: {url}"
    );
}

#[test]
fn no_autospawn_env_keeps_the_missing_daemon_typed() {
    let data = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let output = Command::cargo_bin("austringer")
        .unwrap_or_else(|e| panic!("cargo_bin: {e}"))
        .args([
            "--data-dir",
            data.path().to_str().unwrap_or_default(),
            "capture",
            "hi",
        ])
        .env("AUSTRINGER_NO_AUTOSPAWN", "1")
        .env("AUSTRINGER_DAEMON_BIN", "C:\\definitely\\not\\a-daemon.exe")
        .output()
        .unwrap_or_else(|e| panic!("run cli: {e}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unreachable") && stderr.contains("AUSTRINGER_NO_AUTOSPAWN"),
        "expected the typed refusal, got: {stderr}"
    );
}
