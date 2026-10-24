//! IPC transport integration: the SAME typed protocol over the
//! interprocess local socket (named pipe on Windows), including the
//! token-mismatch refusal and the two-client shared-ask flow.

mod common;

use harness_core::agent_loop::Inference;
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::tokio::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use harness_daemon::ipc::serve_ipc;
use harness_daemon::protocol::SessionEvent;

async fn ipc_connect(name: &str) -> Result<Stream, String> {
    let ns_name = name
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| format!("ns name: {e}"))?;
    Stream::connect(ns_name)
        .await
        .map_err(|e| format!("connect: {e}"))
}

async fn send_line(stream: &mut Stream, json: &str) {
    let _ = stream.write_all(json.as_bytes()).await;
    let _ = stream.write_all(b"\n").await;
    let _ = stream.flush().await;
}

async fn read_lines(
    reader: &mut BufReader<tokio::io::ReadHalf<Stream>>,
    want: &str,
    deadline: Duration,
) -> Vec<String> {
    let mut seen = Vec::new();
    let started = tokio::time::Instant::now();
    while started.elapsed() < deadline {
        let mut line = String::new();
        match tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(_)) => {
                if line.contains(want) {
                    seen.push(line.trim().to_owned());
                    break;
                }
                if !line.trim().is_empty() {
                    seen.push(line.trim().to_owned());
                }
            }
            _ => break,
        }
    }
    seen
}

#[tokio::test]
async fn ipc_hello_with_wrong_token_is_refused() {
    let dir = tempfile::tempdir().ok();
    let Some(dir) = dir else { return };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(common::CompleteOnlyInference));
    let state = match harness_daemon::server::DaemonState::new(
        harness_daemon::config::DaemonConfig {
            data_dir: dir.path().to_owned(),
            token: Some("ipc-token".into()),
            ..harness_daemon::config::DaemonConfig::default()
        },
        factory,
    ) {
        Ok(state) => Arc::new(state),
        Err(_) => return,
    };
    let name = format!("harness-ipc-auth-{}", std::process::id());
    let server = tokio::spawn({
        let state = Arc::clone(&state);
        let name = name.clone();
        async move { serve_ipc(state, &name).await }
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let connect = ipc_connect(&name).await;
    assert!(connect.is_ok(), "ipc connect failed");
    let Ok(mut stream) = connect else {
        server.abort();
        return;
    };
    let hello = serde_json::json!({
        "type": "hello", "version": 1, "token": "WRONG",
        "client_id": "c1", "session": "s1",
    });
    send_line(&mut stream, &hello.to_string()).await;
    let (read_half, _write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);
    let seen = read_lines(&mut reader, "auth", Duration::from_secs(5)).await;
    server.abort();
    assert!(
        seen.iter().any(|l| l.contains("auth")),
        "token mismatch must be typed-refused, saw: {seen:?}"
    );
}

#[tokio::test]
async fn ipc_two_clients_share_one_ask_resolution() {
    let dir = tempfile::tempdir().ok();
    let Some(dir) = dir else { return };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(common::TwoStepInference::new()));
    let state = match harness_daemon::server::DaemonState::new(
        harness_daemon::config::DaemonConfig {
            data_dir: dir.path().to_owned(),
            token: Some("ipc-token".into()),
            ..harness_daemon::config::DaemonConfig::default()
        },
        factory,
    ) {
        Ok(state) => Arc::new(state),
        Err(_) => return,
    };
    // Mock worker so the tool call executes.
    let (worker_tx, mut worker_rx) = tokio::sync::mpsc::channel(4);
    state.worker_router.register(
        "w-ipc",
        vec![harness_daemon::worker::ToolGrant {
            tool: "echo".into(),
        }],
        worker_tx,
    );
    let worker = tokio::spawn({
        let router = Arc::clone(&state.worker_router);
        async move {
            while let Some(call) = worker_rx.recv().await {
                if let harness_daemon::worker::DaemonToWorker::ToolCall { call_id, .. } = call {
                    router.deliver(&call_id, true, serde_json::json!("ipc worker"));
                }
            }
        }
    });

    let name = format!("harness-ipc-two-{}", std::process::id());
    let server = tokio::spawn({
        let state = Arc::clone(&state);
        let name = name.clone();
        async move { serve_ipc(state, &name).await }
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let connect_a = ipc_connect(&name).await;
    assert!(connect_a.is_ok(), "client A connect failed");
    let Ok(mut client_a) = connect_a else {
        server.abort();
        return;
    };
    let connect_b = ipc_connect(&name).await;
    assert!(connect_b.is_ok(), "client B connect failed");
    let Ok(mut client_b) = connect_b else {
        server.abort();
        return;
    };
    let hello = serde_json::json!({
        "type": "hello", "version": 1, "token": "ipc-token",
        "client_id": "ipc-a", "session": "s-ipc",
    });
    send_line(&mut client_a, &hello.to_string()).await;
    let hello_b = serde_json::json!({
        "type": "hello", "version": 1, "token": "ipc-token",
        "client_id": "ipc-b", "session": "s-ipc",
    });
    send_line(&mut client_b, &hello_b.to_string()).await;

    let submit = serde_json::json!({
        "type": "command", "command": {"type": "submit", "text": "go"},
    });
    send_line(&mut client_a, &submit.to_string()).await;

    let (read_a, _w) = tokio::io::split(client_a);
    let mut reader_a = BufReader::new(read_a);
    let (read_b, mut write_b) = tokio::io::split(client_b);
    let mut reader_b = BufReader::new(read_b);

    // Client A answers the first ask it sees; both must observe the
    // same resolution and the same worker output.
    let ask_line = read_lines(&mut reader_a, "permission_ask", Duration::from_secs(10)).await;
    let ask_frame = ask_line.iter().find(|l| l.contains("permission_ask"));
    assert!(ask_frame.is_some(), "no ask observed: {ask_line:?}");
    let Some(ask_frame) = ask_frame else {
        server.abort();
        return;
    };
    let ask_id: u64 = serde_json::from_str::<serde_json::Value>(ask_frame)
        .ok()
        .and_then(|v| {
            v.get("ask")
                .and_then(|a| a.get("ask_id"))
                .and_then(serde_json::Value::as_u64)
        })
        .unwrap_or(0);
    let answer = serde_json::json!({
        "type": "permission_answer", "ask_id": ask_id, "answer": "allow_once",
    });
    // Client B answers (the shared-queue contract: ANY client's answer
    // resolves for everyone).
    let _ = write_b.write_all(answer.to_string().as_bytes()).await;
    let _ = write_b.write_all(b"\n").await;
    let _ = write_b.flush().await;

    let got_a = read_lines(&mut reader_a, "ipc worker", Duration::from_secs(10)).await;
    let got_b = read_lines(&mut reader_b, "ipc worker", Duration::from_secs(10)).await;
    server.abort();
    worker.abort();

    assert!(
        got_a.iter().any(|l| l.contains("ipc worker")),
        "client A missing worker output: {got_a:?}"
    );
    assert!(
        got_b.iter().any(|l| l.contains("ipc worker")),
        "client B missing worker output: {got_b:?}"
    );
    assert!(
        got_a.iter().any(|l| l.contains("permission_resolved")),
        "client A missing resolution: {got_a:?}"
    );
    assert!(
        got_b.iter().any(|l| l.contains("permission_resolved")),
        "client B missing resolution: {got_b:?}"
    );
    // Session continuity after everything.
    let host = state.session("s-ipc");
    host.publish(SessionEvent::Notification {
        kind: "done".into(),
        message: "still attached".into(),
    });
    assert!(host.last_event_id() > 0);
}
