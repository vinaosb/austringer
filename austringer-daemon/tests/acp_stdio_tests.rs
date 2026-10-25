//! Todo 12 (ecosystem alignment): stdio acceptance of `austringer-daemon
//! --acp` — the SHIPPED binary speaking Agent Client Protocol v1 over
//! stdin/stdout against the real in-process session stack (compat
//! fixture provider, CORE_TOOLS registry executors, mediated
//! permission queue). Mirrors the mock-IDE conventions of
//! austringer-mcp/tests/acp_agent.rs at the process boundary.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::header;
use axum::routing::post;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

/// One scripted compat-provider fixture: a FIFO of SSE response
/// bodies served for POST /v1/chat/completions, every captured
/// request body (the @-mention read-path proof reads these), and an
/// optional one-shot gate holding the FIRST response until the test
/// releases it (mid-turn timing control).
struct Fixture {
    scripts: Mutex<VecDeque<String>>,
    bodies: Mutex<Vec<String>>,
    gate: Mutex<Option<Arc<tokio::sync::Notify>>>,
    held: Mutex<Option<Arc<tokio::sync::Notify>>>,
}

impl Fixture {
    async fn answer(&self, body: String) -> String {
        self.bodies.lock().expect("bodies").push(body.clone());
        let hold = self.gate.lock().expect("gate").take();
        if let Some(gate) = hold {
            if let Some(held) = self.held.lock().expect("held").take() {
                held.notify_one();
            }
            gate.notified().await;
        }
        self.scripts
            .lock()
            .expect("scripts")
            .pop_front()
            .expect("fixture script exhausted")
    }
}

/// Renders one scripted turn as chat-completions SSE frames.
fn sse(frames: Vec<Value>) -> String {
    let mut body = String::new();
    for frame in frames {
        body.push_str(&format!("data: {frame}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    body
}

/// A tool-call turn: the assistant edits `path` via edit_lines.
fn tool_call_turn(text: &str, call_id: &str, path: &str, old: &str, new: &str) -> String {
    let arguments = json!({
        "path": path,
        "edits": [{ "line": 1, "old_text": old, "new_text": new }]
    })
    .to_string();
    sse(vec![
        json!({
            "model": "fixture-model",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": text,
                    "tool_calls": [{
                        "index": 0,
                        "id": call_id,
                        "type": "function",
                        "function": { "name": "edit_lines", "arguments": arguments }
                    }]
                }
            }]
        }),
        json!({
            "model": "fixture-model",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "tool_calls" }]
        }),
    ])
}

/// A plain completion turn.
fn text_turn(text: &str) -> String {
    sse(vec![
        json!({
            "model": "fixture-model",
            "choices": [{ "index": 0, "delta": { "content": text } }]
        }),
        json!({
            "model": "fixture-model",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }]
        }),
    ])
}

/// Serves the fixture on an ephemeral loopback port.
async fn spawn_fixture(scripts: Vec<String>) -> (String, Arc<Fixture>) {
    let fixture = Arc::new(Fixture {
        scripts: Mutex::new(scripts.into()),
        bodies: Mutex::new(Vec::new()),
        gate: Mutex::new(None),
        held: Mutex::new(None),
    });
    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post(
                |State(fixture): State<Arc<Fixture>>, body: String| async move {
                    let script = fixture.answer(body).await;
                    ([(header::CONTENT_TYPE, "text/event-stream")], script)
                },
            ),
        )
        .with_state(Arc::clone(&fixture));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fixture bind");
    let addr = listener.local_addr().expect("fixture addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("fixture serve");
    });
    (format!("http://{addr}/v1"), fixture)
}

/// One spawned `austringer-daemon --acp` child: the ACP mock IDE reads
/// its stdout and writes its stdin; killed on drop.
struct AcpChild {
    child: Child,
    stdin: ChildStdin,
    lines: tokio::io::Lines<BufReader<ChildStdout>>,
}

impl Drop for AcpChild {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// Spawns the shipped binary in ACP stdio mode against the fixture
/// provider, rooted at a temp workspace. Relative edit paths resolve
/// against the child's cwd (the IDE host spawns the agent with its
/// workspace as cwd — the process-boundary contract). Returns the
/// child plus that workspace path the test seeds/reads through.
async fn spawn_acp(base_url: &str) -> (AcpChild, PathBuf) {
    let workspace = tempfile::tempdir().expect("workspace");
    let data = tempfile::tempdir().expect("data dir");
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_austringer-daemon"));
    command
        .arg("--acp")
        .arg("--data-dir")
        .arg(data.path())
        .current_dir(workspace.path())
        .env("AUSTRINGER_PROVIDER", "compat")
        .env("AUSTRINGER_MODEL", "fixture-model")
        .env("AUSTRINGER_PROVIDER_BASE_URL", base_url)
        .env_remove("AUSTRINGER_PROVIDER_KEY_ENV")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().expect("spawn --acp");
    // The data dir outlives the test (the provider_binary_tests
    // posture); the workspace is pinned too so the test's file
    // seeds/reads see the same root the daemon resolves relative
    // tool paths against.
    std::mem::forget(data);
    let workspace_path = workspace.path().to_path_buf();
    std::mem::forget(workspace);
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    (
        AcpChild {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
        },
        workspace_path,
    )
}

impl AcpChild {
    /// Sends one JSON-RPC request.
    async fn request(&mut self, id: i64, method: &str, params: Value) {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .expect("write request");
    }

    /// Sends one JSON-RPC notification (no id).
    async fn notify(&mut self, method: &str, params: Value) {
        let line = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        self.stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .expect("write notification");
    }

    /// Reads one stdout line as JSON (deadline-bounded).
    async fn read(&mut self) -> Value {
        let deadline = Duration::from_secs(30);
        let line = tokio::time::timeout(deadline, self.lines.next_line())
            .await
            .expect("read deadline")
            .expect("io")
            .expect("stdout closed");
        serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("non-JSON line {line:?}: {error}"))
    }

    /// Answers one agent→IDE request.
    async fn respond(&mut self, id: Value, result: Value) {
        self.stdin
            .write_all(
                format!(
                    "{}\n",
                    json!({"jsonrpc": "2.0", "id": id, "result": result})
                )
                .as_bytes(),
            )
            .await
            .expect("write response");
    }

    /// Drives `initialize` + `session/new`; returns the new session id.
    async fn initialize_and_new_session(&mut self) -> String {
        self.request(
            1,
            "initialize",
            json!({"protocolVersion": 1, "clientCapabilities": {}, "clientInfo": null}),
        )
        .await;
        loop {
            let message = self.read().await;
            if message.get("id") == Some(&json!(1)) {
                assert_eq!(message["result"]["protocolVersion"], 1, "{message}");
                assert_eq!(message["result"]["agentInfo"]["name"], "austringer");
                break;
            }
        }
        self.request(2, "session/new", json!({"cwd": "."})).await;
        loop {
            let message = self.read().await;
            if message.get("id") == Some(&json!(2)) {
                return message["result"]["sessionId"]
                    .as_str()
                    .expect("sessionId")
                    .to_owned();
            }
        }
    }

    /// Sends `session/prompt` and pumps the wire until its response
    /// arrives, serving agent→IDE requests along the way:
    /// `fs/read_text_file` answers with `mention body`;
    /// `session/request_permission` answers with `chosen_option`
    /// (`None` = the IDE stays silent on that ask). Returns the stop
    /// reason plus every `session/update` payload seen.
    async fn prompt_collect(
        &mut self,
        id: i64,
        session: &str,
        prompt: Value,
        chosen_option: Option<&str>,
    ) -> (String, Vec<Value>) {
        self.request(
            id,
            "session/prompt",
            json!({"sessionId": session, "prompt": prompt}),
        )
        .await;
        let mut updates = Vec::new();
        loop {
            let message = self.read().await;
            match message.get("method").and_then(Value::as_str) {
                Some("fs/read_text_file") => {
                    let read_id = message["id"].clone();
                    self.respond(read_id, json!({"content": "mention body"}))
                        .await;
                }
                Some("session/request_permission") => {
                    if let Some(option) = chosen_option {
                        let ask_id = message["id"].clone();
                        self.respond(
                            ask_id,
                            json!({"outcome": {"outcome": "selected", "optionId": option}}),
                        )
                        .await;
                    }
                }
                Some("session/update") => updates.push(message["params"]["update"].clone()),
                _ => {}
            }
            let is_response = message.get("result").is_some() || message.get("error").is_some();
            if is_response && message.get("id") == Some(&json!(id)) {
                let reason = message["result"]["stopReason"]
                    .as_str()
                    .map_or_else(|| format!("{message}"), str::to_owned);
                return (reason, updates);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acp_stdio_roundtrip_streams_updates_and_asks_permission() {
    let (base, fixture) = spawn_fixture(vec![
        tool_call_turn(
            "editing now",
            "call-1",
            "fixture.md",
            "old line",
            "new line",
        ),
        text_turn("edit applied"),
    ])
    .await;
    let (mut acp, workspace) = spawn_acp(&base).await;
    let session = acp.initialize_and_new_session().await;
    let fixture_file = workspace.join("fixture.md");
    std::fs::write(&fixture_file, "old line\n").expect("seed fixture file");

    let (reason, updates) = acp
        .prompt_collect(
            3,
            &session,
            json!([
                {"type": "text", "text": "please edit the fixture"},
                {"type": "resource_link", "name": "notes", "uri": "notes.txt"}
            ]),
            Some("allow_once"),
        )
        .await;

    // The prompt response ends the turn.
    assert_eq!(reason, "end_turn", "updates: {updates:?}");
    // The assistant text streamed as an agent_message_chunk.
    assert!(
        updates
            .iter()
            .any(|u| u["sessionUpdate"] == "agent_message_chunk"
                && u["content"]["text"] == "editing now"),
        "updates: {updates:?}"
    );
    // The edit proposal streamed as a tool_call carrying diff content.
    let diff = updates.iter().find(|u| {
        u["sessionUpdate"] == "tool_call" && u["content"].as_array().is_some_and(|c| !c.is_empty())
    });
    let diff = diff.expect("a tool_call update with diff content");
    assert_eq!(diff["kind"], "edit", "diff: {diff}");
    assert_eq!(diff["content"][0]["type"], "diff", "diff: {diff}");
    assert_eq!(diff["content"][0]["path"], "fixture.md", "diff: {diff}");
    assert_eq!(diff["content"][0]["oldText"], "old line", "diff: {diff}");
    assert_eq!(diff["content"][0]["newText"], "new line", "diff: {diff}");
    // The allow_once answer admitted the edit through the SAME
    // mediation queue — the file changed on disk.
    let edited = std::fs::read_to_string(&fixture_file).expect("read back");
    assert!(edited.contains("new line"), "edited: {edited:?}");
    // The @-mention rode fs/read_text_file and its content reached
    // the provider request (the read-path proof).
    let bodies = fixture.bodies.lock().expect("bodies").clone();
    assert!(
        bodies.iter().any(|body| body.contains("mention body")),
        "mention content never reached the provider: {bodies:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acp_stdio_denied_permission_fails_closed() {
    let (base, _fixture) = spawn_fixture(vec![
        tool_call_turn(
            "editing now",
            "call-1",
            "protected.md",
            "old line",
            "new line",
        ),
        text_turn("denied, moving on"),
    ])
    .await;
    let (mut acp, workspace) = spawn_acp(&base).await;
    let session = acp.initialize_and_new_session().await;
    let protected = workspace.join("protected.md");
    std::fs::write(&protected, "old line\n").expect("seed protected file");

    let (reason, updates) = acp
        .prompt_collect(
            3,
            &session,
            json!([{"type": "text", "text": "edit the protected file"}]),
            Some("reject_once"),
        )
        .await;

    // The denial flowed through the mediation queue (reject_once was
    // served and consumed); the turn still ends (split-brain preserve)
    // and the tool NEVER executed — deny-fails-closed.
    assert_eq!(reason, "end_turn", "updates: {updates:?}");
    let on_disk = std::fs::read_to_string(&protected).expect("read back");
    assert_eq!(on_disk, "old line\n", "denied edit must not touch the file");
    // The proposal itself streamed (the IDE saw what was refused).
    assert!(
        updates.iter().any(|u| u["sessionUpdate"] == "tool_call"),
        "updates: {updates:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acp_stdio_cancel_lands_through_the_bridge() {
    // Turn 1 runs long (the fixture holds its response); the IDE
    // sends session/cancel mid-turn. The daemon's driver defers a
    // queued Cancel to the turn boundary (pre-existing wire-client
    // semantics — WS/SSE/IPC share them), so the sticky token lands
    // on the NEXT prompt. This proves the bridge forwards
    // session/cancel into the daemon Cancel path and maps the
    // cancelled outcome to its ACP stop reason.
    let gate = Arc::new(tokio::sync::Notify::new());
    let held = Arc::new(tokio::sync::Notify::new());
    let (base, fixture) =
        spawn_fixture(vec![text_turn("slow turn done"), text_turn("second")]).await;
    *fixture.gate.lock().expect("gate") = Some(Arc::clone(&gate));
    *fixture.held.lock().expect("held") = Some(Arc::clone(&held));
    let (mut acp, _workspace) = spawn_acp(&base).await;
    let session = acp.initialize_and_new_session().await;

    // Turn 1: prompt, then cancel MID-TURN. Waiting on `held` (fired by
    // the fixture right before it blocks on the gate) makes mid-turn
    // deterministic - a fixed sleep raced the turn's start on loaded
    // runners, and a cancel landing before the turn is cancellable
    // leaves no sticky token for turn 2.
    acp.request(
        3,
        "session/prompt",
        json!({"sessionId": session, "prompt": [{"type": "text", "text": "go"}]}),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(10), held.notified())
        .await
        .expect("fixture must reach the held turn");
    acp.notify("session/cancel", json!({"sessionId": session}))
        .await;
    // notify_one (not notify_waiters): a stored permit also releases a
    // handler that has not reached `notified().await` yet.
    gate.notify_one();
    let reason = loop {
        let message = acp.read().await;
        let is_response = message.get("result").is_some() || message.get("error").is_some();
        if is_response && message.get("id") == Some(&json!(3)) {
            break message["result"]["stopReason"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
        }
    };
    assert_eq!(reason, "end_turn", "turn 1 completes (cancel deferred)");
    let (reason2, _) = acp
        .prompt_collect(
            4,
            &session,
            json!([{"type": "text", "text": "again"}]),
            None,
        )
        .await;
    assert_eq!(
        reason2, "cancelled",
        "the sticky cancel must land on turn 2"
    );
}
