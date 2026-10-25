//! End-to-end against the REAL austringer-daemon binary (plan todo 39 QA:
//! scripted headless run, detach -> later attach replays missed turns,
//! typed auth failure, the default IPC transport, and doctor's live
//! happy path). The binary's inference is the todo-40 stub, so turn
//! outcomes are deterministically `failed::InferenceExhausted` — the
//! assertions target the CLI mechanics and the OBSERVED session output,
//! never a bare exit code.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::Command as AssertCommand;
use predicates::prelude::*;

struct Daemon {
    child: Child,
    _dir: tempfile::TempDir,
    port: u16,
    token: String,
    ipc_name: String,
    data_dir: PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn daemon_exe() -> PathBuf {
    if let Some(path) = std::env::var_os("AUSTRINGER_DAEMON_EXE") {
        return PathBuf::from(path);
    }
    // Test binaries live in {target}/{profile}/deps; the daemon bin sits
    // one level up next to that directory.
    let exe = if cfg!(windows) {
        "austringer-daemon.exe"
    } else {
        "austringer-daemon"
    };
    let mut probe = std::env::current_exe().unwrap_or_else(|e| panic!("current_exe: {e}"));
    for _ in 0..4 {
        probe.pop();
        if probe.join(exe).is_file() {
            return probe.join(exe);
        }
    }
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "austringer-daemon",
            "--bin",
            "austringer-daemon",
            "--offline",
        ])
        .status()
        .unwrap_or_else(|e| panic!("cargo build austringer-daemon: {e}"));
    assert!(
        status.success(),
        "building austringer-daemon for e2e failed"
    );
    let mut built = std::env::current_exe().unwrap_or_else(|e| panic!("current_exe: {e}"));
    built.pop();
    built.pop();
    built.join(exe)
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map_or(59998, |a| a.port())
}

fn spawn_daemon() -> Daemon {
    static IPC_SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = IPC_SERIAL.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let exe = daemon_exe();
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let data_dir = dir.path().join("d");
    std::fs::create_dir_all(&data_dir).unwrap_or_default();
    // Unique per daemon: parallel tests in this process must never
    // share a pipe name.
    let ipc_name = format!("austringer-cli-e2e-{}-{serial}", std::process::id());
    for _ in 0..3 {
        let port = free_port();
        let mut child = Command::new(&exe)
            .arg("--data-dir")
            .arg(&data_dir)
            .arg("--port")
            .arg(port.to_string())
            .arg("--ipc-name")
            .arg(&ipc_name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {}: {e}", exe.display()));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Ok(token) = std::fs::read_to_string(data_dir.join("daemon-token"))
                && !token.is_empty()
            {
                return Daemon {
                    child,
                    _dir: dir,
                    port,
                    token,
                    ipc_name,
                    data_dir,
                };
            }
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    panic!("daemon never became ready");
}

fn austringer() -> AssertCommand {
    AssertCommand::cargo_bin("austringer").unwrap_or_else(|e| panic!("cargo_bin austringer: {e}"))
}

fn url(d: &Daemon) -> String {
    format!("http://127.0.0.1:{}", d.port)
}

fn cwd_quarantine() -> tempfile::TempDir {
    // Doctor scans the cwd for MCP/skills config: never the repo itself.
    tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"))
}

fn on_session(base: &str, token: &str, session: &str, cmd: &str) -> AssertCommand {
    let mut command = austringer();
    command
        .arg(cmd)
        .arg("--url")
        .arg(base)
        .arg("--token")
        .arg(token)
        .arg("--session")
        .arg(session);
    command
}

#[test]
fn capture_then_attach_replays_missed_turns_over_http() {
    let daemon = spawn_daemon();
    let base = url(&daemon);
    // One session per detached capture: the stub engine is one-shot
    // after a failed run (a second turn on the same session finishes
    // `failed::Terminal` without recording its prompt), so prompt
    // fidelity is proven per session and the replay contract per
    // attach.
    for (session, prompt) in [("rep1", "one alpha"), ("rep2", "two beta")] {
        on_session(&base, &daemon.token, session, "capture")
            .arg(prompt)
            .assert()
            .success()
            .stdout(predicate::str::contains("queued"));
        let output = on_session(&base, &daemon.token, session, "attach")
            .arg("--after")
            .arg("0")
            .arg("--max-events")
            .arg("3")
            .arg("--output-json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8_lossy(&output).into_owned();
        let mut saw_prompt = false;
        let mut finished = 0;
        for line in text.lines().filter(|l| !l.is_empty()) {
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|e| panic!("ndjson line: {e}: {line}"));
            if line.contains(prompt) {
                saw_prompt = true;
            }
            if line.contains("turn_finished") {
                finished += 1;
                assert!(
                    line.contains("failed::InferenceExhausted"),
                    "binary stub outcome expected, saw: {line}"
                );
            }
        }
        assert!(
            saw_prompt,
            "replay must OBSERVE the captured prompt {prompt:?}: {text}"
        );
        assert_eq!(finished, 1, "the detached turn must replay: {text}");
    }
}

#[test]
fn wrong_token_is_a_typed_auth_failure() {
    let daemon = spawn_daemon();
    on_session(&url(&daemon), "wrong-token-entirely", "auth", "attach")
        .arg("--after")
        .arg("0")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("auth"));
}

#[test]
fn ipc_default_transport_captures_and_replays_without_a_url() {
    let daemon = spawn_daemon();
    austringer()
        .arg("capture")
        .arg("--data-dir")
        .arg(&daemon.data_dir)
        .arg("--ipc-name")
        .arg(&daemon.ipc_name)
        .arg("--session")
        .arg("ipc")
        .arg("héllo 🌏 over ipc")
        .assert()
        .success();
    let output = austringer()
        .arg("attach")
        .arg("--data-dir")
        .arg(&daemon.data_dir)
        .arg("--ipc-name")
        .arg(&daemon.ipc_name)
        .arg("--session")
        .arg("ipc")
        .arg("--max-events")
        .arg("3")
        .arg("--output-json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).into_owned();
    let decoded: Vec<String> = text
        .lines()
        .filter(|l| l.contains("héllo 🌏 over ipc"))
        .map(str::to_owned)
        .collect();
    // At-least-once, not exactly-once: the daemon's stub engine can
    // duplicate the recorded user frame when the attach races a
    // running turn (observed: seq 1..3 for one submit). The CLI-owned
    // contract is verbatim unicode replay, not daemon-side dedup.
    assert!(
        !decoded.is_empty(),
        "unicode prompt must replay verbatim: {text}"
    );
    assert_eq!(decoded[0], decoded[decoded.len() - 1]);
}

#[test]
fn doctor_passes_against_a_live_daemon() {
    let daemon = spawn_daemon();
    let guard = cwd_quarantine();
    austringer()
        .current_dir(guard.path())
        .arg("doctor")
        .arg("--url")
        .arg(url(&daemon))
        .arg("--token")
        .arg(&daemon.token)
        .assert()
        .success()
        .stdout(predicate::str::contains("daemon"))
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn run_against_the_stub_binary_reports_the_typed_outcome() {
    let daemon = spawn_daemon();
    on_session(&url(&daemon), &daemon.token, "stub", "run")
        .arg("--output-json")
        .arg("anything")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("failed::InferenceExhausted"));
}
