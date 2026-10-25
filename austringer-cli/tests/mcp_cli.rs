//! assert_cmd suite for the `austringer mcp` subcommand family (plan
//! todo 7): `list`/`scan` render a fixture directory scan, `serve`
//! answers `initialize` over real stdio and exits on EOF, `call`
//! smoke-routes one configured server, and a malformed config fails
//! TYPED (file + key named, never a panic dump). Hermetic: fixture
//! configs live in temp dirs, the only spawned child is this binary
//! itself (`mcp serve` doubles as the call target), zero network,
//! zero API keys.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use predicates::prelude::*;

fn austringer() -> Command {
    Command::cargo_bin("austringer").unwrap_or_else(|e| panic!("cargo_bin austringer: {e}"))
}

/// Writes a fixture `.austringer/mcp.json`: one stdio server, one http
/// server, one trusted-autostart stdio server.
fn fixture_config(dir: &Path) {
    let mcp_dir = dir.join(".austringer");
    std::fs::create_dir_all(&mcp_dir).unwrap_or_default();
    let doc = serde_json::json!({
        "mcpServers": {
            "demo-stdio": { "command": "demo-server", "args": ["--flag"] },
            "demo-http": { "url": "https://example.invalid/mcp" },
            "demo-auto": { "command": "demo-server", "trusted_autostart": true }
        }
    });
    std::fs::write(mcp_dir.join("mcp.json"), doc.to_string()).unwrap_or_default();
}

#[test]
fn mcp_list_renders_fixture_directory_scan() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    fixture_config(dir.path());
    austringer()
        .arg("mcp")
        .arg("list")
        .arg("--dir")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("demo-stdio"))
        .stdout(predicate::str::contains("stdio:demo-server"))
        .stdout(predicate::str::contains("demo-http"))
        .stdout(predicate::str::contains("http:https://example.invalid/mcp"))
        .stdout(predicate::str::contains("austringer"))
        .stdout(predicate::str::contains("start:on-first-use"))
        .stdout(predicate::str::contains("demo-auto"))
        .stdout(predicate::str::contains("start:autostart"));
}

#[test]
fn mcp_scan_alias_renders_the_same_surface() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    fixture_config(dir.path());
    austringer()
        .arg("mcp")
        .arg("scan")
        .arg("--dir")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("demo-stdio"))
        .stdout(predicate::str::contains("demo-http"));
}

#[test]
fn malformed_server_config_is_a_typed_error_naming_file_and_key() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let mcp_dir = dir.path().join(".austringer");
    std::fs::create_dir_all(&mcp_dir).unwrap_or_default();
    std::fs::write(
        mcp_dir.join("mcp.json"),
        br#"{"mcpServers": {"broken-entry": {"neither": true}}}"#,
    )
    .unwrap_or_default();
    austringer()
        .arg("mcp")
        .arg("list")
        .arg("--dir")
        .arg(dir.path())
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("mcp error"))
        .stderr(predicate::str::contains("mcp.json"))
        .stderr(predicate::str::contains("broken-entry"));
}

/// Happy QA scenario: serve + initialize roundtrip over a stdio
/// fixture, then EOF terminates the server (exit 0).
#[test]
fn mcp_serve_answers_initialize_over_stdio_then_exits() {
    let mut child = StdCommand::new(env!("CARGO_BIN_EXE_austringer"))
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn mcp serve: {e}"));
    let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("serve stdin"));
    let request = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":"#,
        r#"{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"cli-smoke","version":"0.0.0"}}}"#,
        "\n"
    );
    stdin
        .write_all(request.as_bytes())
        .unwrap_or_else(|e| panic!("write initialize: {e}"));
    stdin
        .flush()
        .unwrap_or_else(|e| panic!("flush initialize: {e}"));
    drop(stdin); // EOF: the server must answer, then exit.

    let stdout = child
        .stdout
        .take()
        .unwrap_or_else(|| panic!("serve stdout"));
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) > 0 {
            let _ = tx.send(line);
        }
    });
    let reply = rx
        .recv_timeout(Duration::from_secs(20))
        .unwrap_or_else(|_| {
            let _ = child.kill();
            panic!("mcp serve never answered initialize");
        });
    assert!(reply.contains("\"serverInfo\""), "reply: {reply}");
    assert!(reply.contains("austringer"), "reply: {reply}");

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .unwrap_or_else(|e| panic!("wait serve: {e}"))
        {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("mcp serve did not exit after stdin EOF");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(status.success(), "mcp serve exit: {status}");
}

/// `call` smoke: the fixture server's command is this binary in
/// `mcp serve` mode — a full client->spawn->initialize->list round
/// trip without any external process or network.
#[test]
fn mcp_call_smoke_round_trips_through_serve() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let mcp_dir = dir.path().join(".austringer");
    std::fs::create_dir_all(&mcp_dir).unwrap_or_default();
    let doc = serde_json::json!({
        "mcpServers": {
            "self-serve": {
                "command": env!("CARGO_BIN_EXE_austringer"),
                "args": ["mcp", "serve"]
            }
        }
    });
    std::fs::write(mcp_dir.join("mcp.json"), doc.to_string()).unwrap_or_default();
    austringer()
        .arg("mcp")
        .arg("call")
        .arg("self-serve")
        .arg("--dir")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("austringer_ask"))
        .stdout(predicate::str::contains("austringer_status"));
}

#[test]
fn mcp_call_unknown_server_is_a_typed_error() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    austringer()
        .arg("mcp")
        .arg("call")
        .arg("nosuch")
        .arg("--dir")
        .arg(dir.path())
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("mcp error"))
        .stderr(predicate::str::contains("nosuch"));
}
