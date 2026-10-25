//! The server's wire loop: reads frames, routes responses to pending
//! requests, answers server-initiated requests with the spec-safe generic
//! null result, and owns death bookkeeping (pending drain + death hook).
//!
//! Background tasks hold `Weak<Inner>` on purpose: a strong reference would
//! keep the writer alive, the open writer would never EOF the reader, and
//! the reader would keep Inner alive — a leak cycle. Upgrades are
//! transient; a failed upgrade means the last handle is gone and there is
//! nobody left to answer.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::AsyncRead;

use tokio::sync::oneshot;

use crate::error::LspError;
use crate::lock;
use crate::native::Inner;
use crate::proto::{FrameCaps, FrameError, read_frame, write_frame};

/// How the transport ended — decides the error drained into pending
/// requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeathCause {
    /// Pipes closed / EOF: generic broken-transport error.
    Closed,
    /// Protocol corruption: [`LspError::MalformedFrame`].
    Malformed(String),
}

impl DeathCause {
    fn to_error(&self) -> LspError {
        match self {
            Self::Closed => LspError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "language server transport closed",
            )),
            Self::Malformed(detail) => LspError::MalformedFrame {
                detail: detail.clone(),
            },
        }
    }
}

pub(crate) async fn read_loop(
    reader: Box<dyn AsyncRead + Unpin + Send>,
    weak: std::sync::Weak<Inner>,
) {
    let mut buffered = tokio::io::BufReader::new(reader);
    let caps = FrameCaps::default();
    let cause = loop {
        let Some(inner) = weak.upgrade() else { return };
        let killed = inner.killed.notified();
        tokio::pin!(killed);
        let frame = tokio::select! {
            biased;
            _ = &mut killed => break DeathCause::Closed,
            frame = read_frame(&mut buffered, &caps) => frame,
        };
        let body = match frame {
            Ok(body) => body,
            Err(FrameError::Io(_))
            | Err(FrameError::TruncatedHeader)
            | Err(FrameError::TruncatedBody) => break DeathCause::Closed,
            Err(other) => break DeathCause::Malformed(other.to_string()),
        };
        let Ok(msg) = serde_json::from_slice::<Value>(&body) else {
            continue;
        };
        let id = msg.get("id").cloned();
        match (id, msg.get("method").and_then(Value::as_str)) {
            (Some(id), Some(_server_request)) => {
                // Spec-safe generic answer to server-initiated requests
                // (workspace/configuration etc.): result null.
                let reply = json!({"jsonrpc": "2.0", "id": id, "result": Value::Null});
                if write_locked(&inner, &reply).await.is_err() {
                    break DeathCause::Closed;
                }
            }
            (Some(id), None) => {
                if let Some(key) = id.as_i64() {
                    let sender = lock(&inner.pending).remove(&key);
                    if let Some(sender) = sender {
                        let _ = sender.send(map_response(&msg));
                    }
                }
            }
            (None, _) => {
                // Server notification: accepted, not surfaced (servers log
                // through stderr, which is captured separately).
            }
        }
    };
    if let Some(inner) = weak.upgrade() {
        die(inner, cause).await;
    }
}

fn map_response(msg: &Value) -> Result<Value, LspError> {
    match msg.get("error") {
        Some(error) => Err(LspError::RequestFailed {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        }),
        None => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
    }
}

pub(crate) async fn write_locked(inner: &Arc<Inner>, envelope: &Value) -> Result<(), LspError> {
    let body = serde_json::to_vec(envelope).map_err(|err| LspError::MalformedFrame {
        detail: err.to_string(),
    })?;
    let mut writer = inner.writer.lock().await;
    write_frame(&mut *writer, &body).await.map_err(LspError::Io)
}

async fn die(inner: Arc<Inner>, cause: DeathCause) {
    inner.dead.store(true, std::sync::atomic::Ordering::Release);
    let drained: Vec<oneshot::Sender<Result<Value, LspError>>> =
        std::mem::take(&mut *lock(&inner.pending))
            .into_values()
            .collect();
    for sender in drained {
        let _ = sender.send(Err(cause.to_error()));
    }
    if let Some(child) = lock(&inner.child).as_mut() {
        let _ = child.start_kill();
    }
    let hook = lock(&inner.on_death).take();
    if let Some(notify) = hook {
        notify();
    }
}
