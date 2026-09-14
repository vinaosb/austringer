//! Daemon-side fixtures for the e2e driver: scripted two-step
//! inference, a mock diff-producing worker, and the serve mode.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use harness_core::agent_loop::inference::{PendingCall, TASK_COMPLETED_TOOL};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::ContentBlock;
use harness_daemon::config::DaemonConfig;
use harness_daemon::server::{DaemonState, build_router};
use harness_daemon::worker::{DaemonToWorker, ToolGrant};

/// Two-step scripted inference: one `echo` tool call (drives the ask,
/// the worker round-trip, and a Diff event), then task completion.
pub(crate) struct TwoStepInference {
    pub(crate) used: std::sync::atomic::AtomicBool,
}

#[async_trait]
impl Inference for TwoStepInference {
    async fn infer(
        &mut self,
        _request: &harness_core::agent_loop::InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, harness_core::agent_loop::InferenceError> {
        let blocks = |body: &str| {
            vec![ContentBlock::Text {
                text: body.to_owned(),
            }]
        };
        if self.used.swap(true, std::sync::atomic::Ordering::SeqCst) {
            Ok(AssistantTurn {
                blocks: blocks("done: turn completed"),
                tool_calls: vec![PendingCall {
                    call_id: "done".into(),
                    tool: TASK_COMPLETED_TOOL.to_owned(),
                    arguments: serde_json::json!({}),
                }],
            })
        } else {
            Ok(AssistantTurn {
                blocks: blocks("working on it"),
                tool_calls: vec![PendingCall {
                    call_id: "t1".into(),
                    tool: "echo".into(),
                    arguments: serde_json::json!({"q": 1}),
                }],
            })
        }
    }
}

pub(crate) fn factory() -> Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> {
    Arc::new(|| {
        Box::new(TwoStepInference {
            used: false.into(),
        })
    })
}

/// Mock worker: answers `echo` calls with a diff-carrying output so
/// the SPA's hunk-review surface gets real Diff events.
pub(crate) async fn spawn_worker(state: &Arc<DaemonState>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    state.worker_router.register(
        "web-e2e-worker",
        vec![ToolGrant { tool: "echo".into() }],
        tx,
    );
    let router = Arc::clone(&state.worker_router);
    tokio::spawn(async move {
        while let Some(call) = rx.recv().await {
            if let DaemonToWorker::ToolCall { call_id, .. } = call {
                router.deliver(
                    &call_id,
                    true,
                    serde_json::json!({
                        "text": "echoed by mock worker",
                        "diff": {
                            "path": "src/demo.rs",
                            "before_hash": "",
                            "after_hash": "0123456789ab",
                            "hunks": ["@@ -1,2 +1,2 @@\n-old line\n+new line"]
                        }
                    }),
                );
            }
        }
    });
}

/// Serve mode for the playwright driver (until killed).
pub(crate) async fn serve() -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("e2e-token".into()),
        idle_timeout: Duration::from_secs(600),
        ..DaemonConfig::default()
    };
    let state = Arc::new(DaemonState::new(config, factory()).map_err(|e| format!("state: {e}"))?);
    spawn_worker(&state).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18937")
        .await
        .map_err(|e| format!("bind: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .port();
    println!(
        "E2E-DAEMON {}",
        serde_json::json!({"base": format!("127.0.0.1:{port}"), "token": "e2e-token"})
    );
    let app = build_router(Arc::clone(&state));
    let serve = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    );
    std::mem::forget(dir);
    serve.await.map_err(|e| format!("serve: {e}"))
}
