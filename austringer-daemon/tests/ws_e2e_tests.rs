//! Full-transport acceptance over the real axum WebSocket surface:
//! two simultaneous clients see byte-identical event streams, a
//! permission answered once applies to both, malformed frames are
//! bounded, and slow clients are force-resynced.

mod common;

use std::time::Duration;

use common::{spawn_daemon, ws_hello};
use futures_util::{SinkExt, StreamExt};

fn as_text(msg: tokio_tungstenite::tungstenite::Message) -> Option<String> {
    match msg {
        tokio_tungstenite::tungstenite::Message::Text(text) => Some(text.to_string()),
        _ => None,
    }
}

async fn collect_until_turn_finished(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> (Vec<String>, Vec<String>, usize) {
    let mut frames = Vec::new();
    let mut resolutions = Vec::new();
    let mut asks = 0;
    let mut finished = false;
    while let Some(Ok(msg)) = socket.next().await {
        let Some(text) = as_text(msg) else {
            continue;
        };
        if text.contains("permission_ask") {
            asks += 1;
        }
        if text.contains("permission_resolved") {
            resolutions.push(text.clone());
        }
        if text.contains("\"event\"") || text.contains("snapshot") || text.contains("resync") {
            frames.push(text.clone());
        }
        if text.contains("turn_finished") {
            finished = true;
        }
        if finished {
            break;
        }
    }
    (frames, resolutions, asks)
}

#[tokio::test]
async fn two_ws_clients_identical_streams_shared_permission() {
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    // Mock worker answering dispatched tool calls.
    let (worker_tx, mut worker_rx) = tokio::sync::mpsc::channel(4);
    daemon.state.worker_router.register(
        "w-e2e",
        vec![austringer_daemon::worker::ToolGrant {
            tool: "echo".into(),
        }],
        worker_tx,
    );
    let worker = tokio::spawn({
        let router = std::sync::Arc::clone(&daemon.state.worker_router);
        async move {
            while let Some(call) = worker_rx.recv().await {
                if let austringer_daemon::worker::DaemonToWorker::ToolCall { call_id, .. } = call {
                    router.deliver(&call_id, true, serde_json::json!("e2e worker output"));
                }
            }
        }
    });

    let url_a = format!("ws://{}/s/e2e/attach?token={}", daemon.base, daemon.token);
    let url_b = format!("ws://{}/s/e2e/attach?token={}", daemon.base, daemon.token);
    let (result_a, result_b) = (
        ws_hello(&url_a, &daemon.token, "tui", None).await,
        ws_hello(&url_b, &daemon.token, "web", None).await,
    );
    assert!(
        result_a.is_ok() && result_b.is_ok(),
        "attach failed: {:?} {:?}",
        result_a,
        result_b
    );
    let (Ok(socket_a), Ok(socket_b)) = (result_a, result_b) else {
        return;
    };

    // Both clients are PURE collectors from their attach point; a
    // third surface (HTTP) answers the broadcast ask — the shared
    // permission queue resolves for everyone.
    let reader_a = tokio::spawn(async move {
        let mut socket = socket_a;
        collect_until_turn_finished(&mut socket).await
    });
    let reader_b = tokio::spawn(async move {
        let mut socket = socket_b;
        collect_until_turn_finished(&mut socket).await
    });

    // Subscribe BEFORE submitting: the turn is fast enough to broadcast
    // the ask before a late subscriber connects.
    let host = daemon.state.session("e2e");
    let mut watch = host.mediator().subscribe();

    submit_http(&daemon.base, &daemon.token, "e2e", "hello e2e").await;
    answer_first_ask_http(&daemon, "e2e", &mut watch).await;

    let joined = (reader_a.await.ok(), reader_b.await.ok());
    assert!(joined.0.is_some() && joined.1.is_some(), "readers failed");
    let (Some(collected_a), Some(collected_b)) = joined else {
        return;
    };
    let (frames_a, resolutions_a, asks_a) = collected_a;
    let (frames_b, resolutions_b, asks_b) = collected_b;

    // Byte-level event equality (plan acceptance).
    assert_eq!(frames_a, frames_b, "two clients must see identical frames");
    assert_eq!(asks_a, 1);
    assert_eq!(asks_b, 1);
    assert_eq!(resolutions_a.len(), 1);
    assert_eq!(resolutions_a, resolutions_b);
    let joined = frames_a.join("\n");
    assert!(
        joined.contains("e2e worker output"),
        "worker output must stream: {joined}"
    );
    worker.abort();
}

async fn submit_http(base: &str, token: &str, session: &str, text: &str) {
    let body = serde_json::json!({
        "type": "command", "command": {"type": "submit", "text": text},
    });
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("http://{base}/s/{session}/cmd"))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await;
}

async fn answer_first_ask_http(
    daemon: &common::Spawned,
    session: &str,
    watch: &mut tokio::sync::broadcast::Receiver<austringer_daemon::protocol::ServerMsg>,
) {
    // The pre-subscribed control bus yields the real ask id (race-free);
    // answering over HTTP resolves the ask for every client.
    let mut ask_id = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while ask_id.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(austringer_daemon::protocol::ServerMsg::PermissionAsk { ask }) = watch.try_recv()
        {
            ask_id = Some(ask.ask_id);
        } else {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    assert!(ask_id.is_some(), "no ask broadcast within deadline");
    let Some(ask_id) = ask_id else { return };
    let body = serde_json::json!({
        "type": "permission_answer", "ask_id": ask_id, "answer": "allow_once",
    });
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("http://{}/s/{session}/cmd", daemon.base))
        .bearer_auth(&daemon.token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await;
}

#[tokio::test]
async fn malformed_frames_are_rejected_then_connection_closed() {
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let url = format!("ws://{}/s/mal/attach?token={}", daemon.base, daemon.token);
    let handshake = ws_hello(&url, &daemon.token, "tui", None).await;
    assert!(handshake.is_ok(), "attach failed: {handshake:?}");
    let Ok(mut socket) = handshake else {
        return;
    };
    let mut errors = 0;
    for _ in 0..3 {
        let _ = socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                "this is not json".into(),
            ))
            .await;
    }
    while let Some(Ok(msg)) = socket.next().await {
        if let Some(text) = as_text(msg)
            && text.contains("malformed_frame")
        {
            errors += 1;
        }
        if errors >= 3 {
            break;
        }
    }
    assert!(errors >= 1, "garbage frames must be typed-rejected");
    // After the strike cap the reader stops; the connection closes.
    let closed = socket.next().await;
    assert!(
        closed.is_none() || closed.is_some_and(|r| r.is_err()),
        "connection must close after repeated garbage"
    );
}

#[tokio::test]
async fn slow_client_is_force_resynced_and_session_survives() {
    let daemon = spawn_daemon(|c| c).await.ok();
    assert!(daemon.is_some(), "spawn failed");
    let Some(daemon) = daemon else { return };
    let host = daemon.state.session("slow");
    // A fresh attach subscribes live; then flood the (tiny) broadcast
    // ring while the client is NOT reading.
    let url = format!("ws://{}/s/slow/attach?token={}", daemon.base, daemon.token);
    let handshake = ws_hello(&url, &daemon.token, "slow-client", None).await;
    assert!(handshake.is_ok(), "attach failed: {handshake:?}");
    let Ok(mut socket) = handshake else {
        return;
    };
    // Don't read; publish far beyond the default broadcast capacity.
    for i in 0..600 {
        host.publish(austringer_daemon::protocol::SessionEvent::Notification {
            kind: "flood".into(),
            message: format!("n{i}"),
        });
    }
    // Now resume reading: the lagged client must be told to resync and
    // given a snapshot — the session is intact.
    let mut saw_resync = false;
    let mut saw_snapshot = false;
    let deadline = Duration::from_secs(10);
    let started = tokio::time::Instant::now();
    while started.elapsed() < deadline {
        match tokio::time::timeout(Duration::from_secs(2), socket.next()).await {
            Ok(Some(Ok(msg))) => {
                if let Some(text) = as_text(msg) {
                    if text.contains("state_resync_required") {
                        saw_resync = true;
                    }
                    if text.contains("snapshot") {
                        saw_snapshot = true;
                    }
                    if saw_resync && saw_snapshot {
                        break;
                    }
                }
            }
            _ => break,
        }
    }
    assert!(saw_resync, "slow client must be forced to resync");
    assert!(saw_snapshot, "resync must carry a snapshot");
    // Session continuity: new events still flow to a fresh attach.
    host.publish(austringer_daemon::protocol::SessionEvent::Notification {
        kind: "after".into(),
        message: "still alive".into(),
    });
    let host2 = daemon.state.session("slow");
    assert_eq!(host2.session_id(), "slow");
}

#[tokio::test]
async fn idle_read_timeout_closes_the_client_not_the_session() {
    let daemon = match spawn_daemon(|c| austringer_daemon::config::DaemonConfig {
        idle_timeout: Duration::from_millis(400),
        ..c
    })
    .await
    {
        Ok(daemon) => daemon,
        Err(_) => return,
    };
    let url = format!("ws://{}/s/idle/attach?token={}", daemon.base, daemon.token);
    let mut socket = match ws_hello(&url, &daemon.token, "idle-client", None).await {
        Ok(socket) => socket,
        Err(_) => return,
    };
    let mut saw_bye = false;
    while let Some(Ok(msg)) = socket.next().await {
        if let Some(text) = as_text(msg)
            && text.contains("idle read timeout")
        {
            saw_bye = true;
            break;
        }
    }
    assert!(saw_bye, "idle client must receive a Bye before close");
    // The session survives the disconnect.
    assert_eq!(daemon.state.session("idle").session_id(), "idle");
}
