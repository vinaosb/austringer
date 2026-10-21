//! Shared test harness for daemon integration tests: a real axum
//! server on an ephemeral loopback port with scripted inference.

// Shared by multiple integration-test binaries; not every helper is
// used by every binary.
#![allow(dead_code)]

pub mod cassette;

use std::sync::Arc;
use std::time::Duration;

use harness_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::ContentBlock;
use harness_daemon::config::DaemonConfig;
use harness_daemon::server::{DaemonState, build_router};

pub struct Spawned {
    pub base: String,
    pub token: String,
    pub state: Arc<DaemonState>,
}

/// Scripted inference: always a tool-call turn (drives asks + worker
/// execution each turn).
pub struct ToolLoopInference;

#[async_trait::async_trait]
impl Inference for ToolLoopInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "working".into(),
            }],
            tool_calls: vec![PendingCall {
                call_id: "t1".into(),
                tool: "echo".into(),
                arguments: serde_json::json!({"q": 1}),
            }],
            usage: None,
        })
    }
}

/// Completion-only inference (no tools, no asks).
pub struct CompleteOnlyInference;

#[async_trait::async_trait]
impl Inference for CompleteOnlyInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "done".into(),
            }],
            tool_calls: vec![PendingCall {
                call_id: "done".into(),
                tool: TASK_COMPLETED_TOOL.to_owned(),
                arguments: serde_json::json!({}),
            }],
            usage: None,
        })
    }
}

/// Two-step script: one tool-call turn, then task completion. This is
/// the shape integration tests want — exactly one ask, one worker
/// round-trip, then a completed turn.
pub struct TwoStepInference {
    used: std::sync::atomic::AtomicBool,
}

impl TwoStepInference {
    #[must_use]
    pub fn new() -> Self {
        Self {
            used: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for TwoStepInference {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Inference for TwoStepInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        if self.used.swap(true, std::sync::atomic::Ordering::SeqCst) {
            Ok(AssistantTurn {
                blocks: vec![ContentBlock::Text {
                    text: "done".into(),
                }],
                tool_calls: vec![PendingCall {
                    call_id: "done".into(),
                    tool: TASK_COMPLETED_TOOL.to_owned(),
                    arguments: serde_json::json!({}),
                }],
                usage: None,
            })
        } else {
            Ok(AssistantTurn {
                blocks: vec![ContentBlock::Text {
                    text: "working".into(),
                }],
                tool_calls: vec![PendingCall {
                    call_id: "t1".into(),
                    tool: "echo".into(),
                    arguments: serde_json::json!({"q": 1}),
                }],
                usage: None,
            })
        }
    }
}

/// Bind the daemon on an ephemeral loopback port with a fixed token.
/// The server task runs until the test process ends; the data-dir
/// tempdir is intentionally kept alive for the process lifetime.
pub async fn spawn_daemon(
    tweak: impl FnOnce(DaemonConfig) -> DaemonConfig,
) -> Result<Spawned, String> {
    spawn_daemon_with_factory(tweak, {
        let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
            Arc::new(|| Box::new(TwoStepInference::new()));
        factory
    })
    .await
}

/// `spawn_daemon` with the session factory injected — the M32 provider
/// wiring tests compose the production factory (identity-bearing) and
/// plug it in here; scripted tests pass legacy factories.
pub async fn spawn_daemon_with_factory(
    tweak: impl FnOnce(DaemonConfig) -> DaemonConfig,
    factory: impl harness_daemon::usage::IntoSessionFactory,
) -> Result<Spawned, String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("e2e-token".into()),
        idle_timeout: Duration::from_secs(60),
        ..DaemonConfig::default()
    };
    let config = tweak(config);
    let state = Arc::new(DaemonState::new(config, factory).map_err(|e| format!("state: {e}"))?);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("bind: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .port();
    let app = build_router(Arc::clone(&state));
    tokio::spawn(async move {
        let serve = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        let _ = serve.await;
    });
    std::mem::forget(dir);
    Ok(Spawned {
        base: format!("127.0.0.1:{port}"),
        token: state.token.clone(),
        state,
    })
}

/// Connect a WS client and perform the Hello handshake; returns the
/// socket once the first server frame (hello ack / snapshot path)
/// arrives.
pub async fn ws_hello(
    url: &str,
    token: &str,
    client_id: &str,
    session: Option<String>,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    use futures_util::{SinkExt, StreamExt};
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .map_err(|e| format!("ws connect: {e}"))?;
    let hello = serde_json::json!({
        "type": "hello",
        "version": 1,
        "token": token,
        "client_id": client_id,
        "session": session,
    });
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            hello.to_string().into(),
        ))
        .await
        .map_err(|e| format!("hello send: {e}"))?;
    match socket.next().await {
        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
            let accepted = ["hello_ack", "snapshot", "state_resync_required"];
            if accepted.iter().any(|marker| text.contains(marker)) {
                Ok(socket)
            } else {
                Err(format!("unexpected first frame: {text}"))
            }
        }
        other => Err(format!("no hello ack: {other:?}")),
    }
}
