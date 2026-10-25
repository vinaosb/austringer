//! Subprocess acceptance over the SHIPPED daemon binary: the real
//! `austringer-daemon` process, its stdout handshake, the HTTP surface,
//! and the WebSocket protocol — no in-process state shortcuts.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::AsyncBufReadExt;

/// One spawned daemon process: parses the stdout handshake line,
/// keeps the child alive, and kills it on drop.
struct Daemon {
    base: String,
    token: String,
    child: tokio::process::Child,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// Starts the real binary with a temp data dir and a scrubbed provider
/// environment, then waits for the `listening` handshake line.
async fn spawn_binary(args: &[&str]) -> Result<Daemon, String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let data_arg = dir.path().to_string_lossy().to_string();
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_austringer-daemon"));
    cmd.arg("--data-dir").arg(&data_arg);
    for arg in args {
        cmd.arg(arg);
    }
    for var in [
        "austringer_PROVIDER",
        "austringer_MODEL",
        "austringer_PROVIDER_KEY_ENV",
        "austringer_PROVIDER_BASE_URL",
    ] {
        cmd.env_remove(var);
    }
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
    std::mem::forget(dir);
    let stdout = child.stdout.take().ok_or_else(|| "no stdout".to_owned())?;
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    let deadline = Duration::from_secs(60);
    let started = tokio::time::Instant::now();
    while started.elapsed() < deadline {
        let line = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
            .await
            .map_err(|e| format!("read timeout: {e}"))?
            .map_err(|e| format!("read: {e}"))?;
        let Some(line) = line else {
            return Err("stdout closed".to_owned());
        };
        if let Some((base, token)) = parse_handshake(&line) {
            return Ok(Daemon { base, token, child });
        }
    }
    Err("no handshake line within deadline".to_owned())
}

/// `austringer-daemon listening on http://127.0.0.1:PORT (token: TOK)`.
fn parse_handshake(line: &str) -> Option<(String, String)> {
    let rest = line.split("listening on http://").nth(1)?;
    let (base, tail) = rest.split_once(" (token: ")?;
    let token = tail.strip_suffix(')')?;
    Some((base.to_owned(), token.to_owned()))
}

/// Attaches a WS client, submits one prompt over HTTP, and returns every
/// text frame seen up to and including the turn outcome.
async fn attach_submit_collect(
    daemon: &mut Daemon,
    session: &str,
    prompt: &str,
) -> Result<Vec<String>, String> {
    let url = format!(
        "ws://{}/s/{session}/attach?token={}",
        daemon.base, daemon.token
    );
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .map_err(|e| format!("ws connect: {e}"))?;
    let hello = serde_json::json!({
        "type": "hello", "version": 1, "token": daemon.token,
        "client_id": "bin-e2e", "session": None::<String>,
    });
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            hello.to_string().into(),
        ))
        .await
        .map_err(|e| format!("hello: {e}"))?;
    // First frame: hello ack / snapshot.
    let _ = socket
        .next()
        .await
        .ok_or_else(|| "no hello ack".to_owned())?;
    let body = serde_json::json!({
        "type": "command", "command": {"type": "submit", "text": prompt},
    });
    let client = reqwest::Client::new();
    client
        .post(format!("http://{}/s/{session}/cmd", daemon.base))
        .bearer_auth(&daemon.token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| format!("submit: {e}"))?;
    let mut frames = Vec::new();
    let deadline = Duration::from_secs(30);
    while tokio::time::Instant::now().elapsed() < deadline {
        let Ok(Some(Ok(msg))) = tokio::time::timeout(Duration::from_secs(10), socket.next()).await
        else {
            break;
        };
        let tokio_tungstenite::tungstenite::Message::Text(text) = msg else {
            continue;
        };
        let text = text.to_string();
        let finished = text.contains("turn_finished");
        frames.push(text);
        if finished {
            return Ok(frames);
        }
    }
    Err("no turn_finished within deadline".to_owned())
}

#[tokio::test]
async fn unconfigured_daemon_fails_typed_over_the_wire() {
    let mut daemon = match spawn_binary(&[]).await {
        Ok(daemon) => daemon,
        Err(failure) => panic!("spawn failed: {failure}"),
    };
    // Health surface answers while no provider is configured.
    let health = reqwest::Client::new()
        .get(format!("http://{}/", daemon.base))
        .bearer_auth(&daemon.token)
        .send()
        .await;
    assert!(
        health.is_ok_and(|r| r.status().is_success()),
        "root must serve"
    );
    let frames = match attach_submit_collect(&mut daemon, "m32-bin", "hello provider").await {
        Ok(frames) => frames,
        Err(failure) => panic!("round-trip failed: {failure}"),
    };
    let joined = frames.join("\n");
    assert!(
        joined.contains("no provider configured"),
        "typed unconfigured error must reach the wire: {joined}"
    );
    assert!(
        joined.contains("austringer_PROVIDER") && joined.contains("austringer_MODEL"),
        "the error must name the config keys to set: {joined}"
    );
    assert!(
        !joined.contains("todo 40"),
        "the stale 'lands with todo 40' reference must be gone: {joined}"
    );
    assert!(
        joined.contains("turn_finished"),
        "the turn must terminate, never hang"
    );
}
