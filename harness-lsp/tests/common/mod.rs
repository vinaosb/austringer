//! The in-test mock language server: a thread-task speaking real
//! Content-Length JSON-RPC over simplex pipes. Proves routing, framing,
//! restart, and degradation paths without needing gopls/tsserver installed
//! anywhere.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use harness_lsp::proto::{FrameCaps, read_frame, write_frame};
use serde_json::{Value, json};
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::simplex;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// How the mock answers one client request.
#[derive(Clone, Debug)]
pub enum Action {
    /// Respond with a JSON-RPC result.
    Result(Value),
    /// Respond with a JSON-RPC error object (code only).
    Error(i64),
    /// Never answer (hung server).
    Hang,
    /// Close the connection immediately (crashed server).
    Close,
    /// Sleep, then respond (slow but eventual server).
    DelayedResult(Duration, Value),
}

/// Handler deciding the answer for each incoming request.
pub type Handler = Arc<dyn Fn(&str, &Value) -> Action + Send + Sync>;

/// What the mock saw: (method, full message). `method` is empty for
/// responses to mock-initiated requests.
pub type Seen = (String, Value);

/// Client-side ends of the pipe, ready for `ServerHandle::attach`.
pub type ClientEnds = (
    Box<dyn AsyncRead + Unpin + Send>,
    Box<dyn AsyncWrite + Unpin + Send>,
);

/// Capabilities a cooperative server reports (UTF-8 positions, like
/// rust-analyzer does when asked).
pub fn utf8_caps() -> Value {
    json!({"capabilities": {"positionEncoding": "utf-8"}})
}

/// Capabilities of a UTF-16-only server (e.g. typescript-language-server).
pub fn utf16_caps() -> Value {
    json!({"capabilities": {}})
}

/// A canned single-location navigation result.
pub fn location_result(uri: &str, line0: u64, character0: u64) -> Value {
    json!({
        "uri": uri,
        "range": {"start": {"line": line0, "character": character0},
                  "end":   {"line": line0, "character": character0}}
    })
}

/// The mock server handle: what tests use to observe and perturb it.
pub struct MockLsp {
    /// Everything the mock received, in order.
    pub seen: mpsc::UnboundedReceiver<Seen>,
    wire: Arc<Mutex<tokio::io::WriteHalf<tokio::io::SimplexStream>>>,
    kill: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl MockLsp {
    /// Sends an arbitrary message as a framed JSON value (used for
    /// server-initiated requests/notifications).
    pub async fn send(&self, value: &Value) {
        let body = serde_json::to_vec(value).expect("test json");
        let mut wire = self.wire.lock().await;
        write_frame(&mut *wire, &body).await.expect("test write");
    }

    /// Writes raw bytes with NO framing — garbage on the server's stdout.
    pub async fn send_raw(&self, bytes: &[u8]) {
        let mut wire = self.wire.lock().await;
        wire.write_all(bytes).await.expect("test write");
        wire.flush().await.expect("test flush");
    }

    /// Waits until a message with this method name was seen (draining
    /// others); returns the full message. Panics after 10 s — tests only
    /// wait for things their handler definitely produces.
    pub async fn wait_for_method(&mut self, method: &str) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let (_, msg) = tokio::time::timeout_at(deadline, self.seen.recv())
                .await
                .expect("mock saw no more messages")
                .expect("mock channel closed");
            if msg.get("method").and_then(Value::as_str) == Some(method) {
                return msg;
            }
        }
    }

    /// Waits until a response (id-only message) arrives whose id matches.
    pub async fn wait_for_response(&mut self, id: &Value) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let (_, msg) = tokio::time::timeout_at(deadline, self.seen.recv())
                .await
                .expect("mock saw no more messages")
                .expect("mock channel closed");
            if msg.get("method").is_none() && msg.get("id") == Some(id) {
                return msg;
            }
        }
    }

    /// Crashes the server: the pipe dies on both ends (kill -9 semantics
    /// as seen from the client transport).
    pub fn crash(&mut self) {
        if let Some(kill) = self.kill.take() {
            let _ = kill.send(());
        }
    }

    /// Awaits the serve task's end (after crash or client hang-up).
    pub async fn join(self) {
        let _ = self.task.await;
    }
}

/// Spawns the mock server and returns the client-side pipe ends plus the
/// control handle.
///
/// Two unidirectional simplex pipes (not one duplex) so that dropping
/// either side's writer reliably EOFs the peer's reader — the property the
/// graceful-shutdown tests depend on.
pub async fn spawn_mock(handler: Handler) -> (ClientEnds, MockLsp) {
    const PIPE: usize = 256 * 1024;
    let (server_read, client_writer) = simplex(PIPE);
    let (client_read, server_writer) = simplex(PIPE);

    let (seen_tx, seen_rx) = mpsc::unbounded_channel();
    let (kill_tx, kill_rx) = oneshot::channel();
    let wire = Arc::new(Mutex::new(server_writer));

    let task = tokio::spawn(serve(
        server_read,
        Arc::clone(&wire),
        handler,
        seen_tx,
        kill_rx,
    ));

    (
        (Box::new(client_read), Box::new(client_writer)),
        MockLsp {
            seen: seen_rx,
            wire,
            kill: Some(kill_tx),
            task,
        },
    )
}

async fn serve(
    read: tokio::io::ReadHalf<tokio::io::SimplexStream>,
    wire: Arc<Mutex<tokio::io::WriteHalf<tokio::io::SimplexStream>>>,
    handler: Handler,
    seen: mpsc::UnboundedSender<Seen>,
    mut kill: oneshot::Receiver<()>,
) {
    let mut reader = BufReader::new(read);
    let caps = FrameCaps::default();
    loop {
        let body = tokio::select! {
            biased;
            _ = &mut kill => break,
            frame = read_frame(&mut reader, &caps) => match frame {
                Ok(body) => body,
                Err(_) => break,
            },
        };
        let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
            continue;
        };
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let _ = seen.send((method.clone(), msg.clone()));

        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let is_server_response = method.is_empty() && msg.get("id").is_some();
        let action = handler(&method, &params);
        if matches!(action, Action::Close) {
            break;
        }
        if let Some(id) = msg.get("id").cloned()
            && !is_server_response
        {
            match action {
                Action::Result(v) => respond(&wire, &id, Ok(v)).await,
                Action::Error(code) => respond(&wire, &id, Err(code)).await,
                Action::Hang => {}
                Action::DelayedResult(delay, v) => {
                    tokio::time::sleep(delay).await;
                    respond(&wire, &id, Ok(v)).await;
                }
                Action::Close => {}
            }
        }
    }
    // Simplex halves do NOT propagate EOF on drop — an explicit shutdown is
    // the only way the client's reader observes the "process death".
    let mut writer = wire.lock().await;
    let _ = writer.shutdown().await;
}

async fn respond(
    wire: &Arc<Mutex<tokio::io::WriteHalf<tokio::io::SimplexStream>>>,
    id: &Value,
    result: Result<Value, i64>,
) {
    let reply = match result {
        Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
        Err(code) => json!({"jsonrpc": "2.0", "id": id,
                             "error": {"code": code, "message": "mock error"}}),
    };
    let body = serde_json::to_vec(&reply).expect("test json");
    let mut wire = wire.lock().await;
    let _ = write_frame(&mut *wire, &body).await;
}
