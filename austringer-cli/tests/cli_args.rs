//! assert_cmd suite (plan todo 39 acceptance): argument surface,
//! version embed, typed failures for bad args, missing images, absent
//! daemons, and doctor's induced-failure exits. Everything here is
//! hermetic: no daemon, no network, no TTY.

use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn austringer() -> Command {
    Command::cargo_bin("austringer").unwrap_or_else(|e| panic!("cargo_bin austringer: {e}"))
}

fn unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map_or(59999, |a| a.port())
}

#[test]
fn help_lists_the_full_surface() {
    austringer()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("capture"))
        .stdout(predicate::str::contains("repl"))
        .stdout(predicate::str::contains("attach"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("mcp"))
        .stdout(predicate::str::contains("--output-json"))
        .stdout(predicate::str::contains("--image"));
}

#[test]
fn version_prints_the_workspace_version() {
    austringer()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn bad_subcommand_is_a_clap_usage_error() {
    austringer()
        .arg("frobnicate")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn bare_austringer_execs_the_tui_front_door() {
    // No TUI installed on the test PATH and no override: the front
    // door fails with the typed not-found error naming the binary.
    austringer()
        .env(
            "AUSTRINGER_TUI_BIN",
            "C:\\definitely\\not\\austringer-tui.exe",
        )
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("austringer-tui"))
        .stderr(predicate::str::contains("AUSTRINGER_TUI_BIN"));
}

#[test]
fn run_without_prompt_is_a_usage_error() {
    austringer().arg("run").assert().failure().code(2);
}

#[test]
fn missing_image_fails_typed_before_any_daemon_contact() {
    austringer()
        .arg("run")
        .arg("--url")
        .arg("http://127.0.0.1:1")
        .arg("--image")
        .arg("definitely-not-here.png")
        .arg("hi")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("image"));
}

#[test]
fn non_image_attachment_is_rejected_typed() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, b"plain text, not an image").unwrap_or_default();
    austringer()
        .arg("capture")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{}", unused_port()))
        .arg("--image")
        .arg(&path)
        .arg("hi")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("image"));
}

#[test]
fn daemon_absent_is_a_typed_failure_for_run() {
    austringer()
        .arg("run")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{}", unused_port()))
        .arg("hi")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("unreachable"));
}

#[test]
fn daemon_absent_is_a_typed_failure_for_capture() {
    austringer()
        .arg("capture")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{}", unused_port()))
        .arg("hi")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("unreachable"));
}

#[test]
fn doctor_exits_nonzero_when_daemon_absent() {
    austringer()
        .arg("doctor")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{}", unused_port()))
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("daemon"))
        .stdout(predicate::str::contains("FAIL"));
}

#[test]
fn doctor_exits_nonzero_on_broken_provider_selection() {
    austringer()
        .arg("doctor")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{}", unused_port()))
        .env("AUSTRINGER_PROVIDER", "not-a-provider")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("provider"))
        .stdout(predicate::str::contains("FAIL"));
}

#[test]
fn doctor_exits_nonzero_on_malformed_mcp_config() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let mcp_dir = dir.path().join(".austringer");
    std::fs::create_dir_all(&mcp_dir).unwrap_or_default();
    std::fs::write(mcp_dir.join("mcp.json"), b"{ not json").unwrap_or_default();
    let port = unused_port();
    austringer()
        .current_dir(dir.path())
        .arg("doctor")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{port}"))
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("mcp"))
        .stdout(predicate::str::contains("FAIL"));
}

#[test]
fn doctor_exits_nonzero_on_corrupt_sessions_store() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    std::fs::write(dir.path().join("store.db"), b"definitely not sqlite").unwrap_or_default();
    austringer()
        .arg("doctor")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{}", unused_port()))
        .arg("--sessions")
        .arg(dir.path())
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("sessions"))
        .stdout(predicate::str::contains("FAIL"));
}

#[test]
fn unicode_prompt_survives_argument_handling() {
    // No daemon: the typed unreachable error proves the unicode prompt
    // was accepted by the parser and reached the connect stage.
    austringer()
        .arg("run")
        .arg("--url")
        .arg(format!("http://127.0.0.1:{}", unused_port()))
        .arg("héllo 🌏 Привет")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("unreachable"));
}

#[test]
fn image_helper_rejects_directories_typed() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let path: PathBuf = dir.path().to_path_buf();
    assert!(
        austringer_cli::image::attachments(std::slice::from_ref(&path), false)
            .unwrap_err()
            .to_string()
            .contains("image")
    );
}
