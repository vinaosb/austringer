//! `austringer web` end to end: with no daemon running it spawns the
//! real one, prints the dashboard URL, and (--no-browser) never
//! touches a browser. The spawned daemon is killed by pid at the end.

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

fn arm_watchdog(seconds: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        eprintln!("web e2e: wedged past {seconds}s; aborting");
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
fn web_with_no_daemon_spawns_one_and_prints_the_dashboard_url() {
    arm_watchdog(90);
    let data = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let ipc = format!("web-test-{}", std::process::id());
    // stderr to a FILE: the detached daemon must never hold a pipe of
    // ours (the austringer-launch handle-inheritance lesson).
    let stderr_log = data.path().join("cli-stderr.log");
    let stderr_file = std::fs::File::create(&stderr_log).unwrap_or_else(|e| panic!("stderr: {e}"));
    let output = Command::cargo_bin("austringer")
        .unwrap_or_else(|e| panic!("cargo_bin: {e}"))
        .args([
            "--data-dir",
            data.path().to_str().unwrap_or_default(),
            "--ipc-name",
            &ipc,
            "web",
            "--no-browser",
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
        stdout.contains("austringer web: http://127.0.0.1:"),
        "expected the dashboard URL on stdout, got: {stdout} stderr: {stderr}"
    );
}
