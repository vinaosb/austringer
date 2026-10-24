//! Daemon-side fixtures for the e2e driver: scripted two-step
//! inference (identity-bearing so v2 Usage frames flow), a mock
//! diff-producing worker, fixture team frames, and the serve mode.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use harness_core::agent_loop::inference::{PendingCall, TASK_COMPLETED_TOOL, TurnUsage};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::ContentBlock;
use harness_daemon::automations::NetworkPolicy;
use harness_daemon::config::DaemonConfig;
use harness_daemon::events::SessionEvent;
use harness_daemon::run_history::{RunHistory, RunRecord};
use harness_daemon::server::{DaemonState, build_router};
use harness_daemon::usage::{ComposedInference, InferenceFactory};
use harness_daemon::worker::{DaemonToWorker, ToolGrant};
use harness_providers::identity::ModelIdentity;

/// The fixture identity: priced in the builtin tables, so every
/// scripted turn books a deterministic micro-USD cost (6720 per
/// inference under the usage below).
const FIXTURE_IDENTITY: (&str, &str) = ("anthropic", "claude-sonnet-4-5");

/// Two-step scripted inference: one `echo` tool call (drives the ask,
/// the worker round-trip, and a Diff event), then task completion.
/// Reports usage on every step so the v2 Usage frames the dashboard's
/// cost/trend surfaces consume actually flow in e2e.
pub(crate) struct TwoStepInference {
    pub(crate) used: std::sync::atomic::AtomicBool,
}

fn fixture_usage() -> Option<TurnUsage> {
    Some(TurnUsage {
        input_tokens: 1200,
        output_tokens: 80,
        cache_read_tokens: 6400,
        cache_write_tokens: 0,
    })
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
                usage: fixture_usage(),
            })
        } else {
            Ok(AssistantTurn {
                blocks: blocks("working on it"),
                tool_calls: vec![PendingCall {
                    call_id: "t1".into(),
                    tool: "echo".into(),
                    arguments: serde_json::json!({"q": 1}),
                }],
                usage: fixture_usage(),
            })
        }
    }
}

/// Identity-bearing factory: the daemon wraps the stack with the
/// Usage-frame decorator, so attached v2 clients see per-turn cost.
pub(crate) fn factory() -> InferenceFactory {
    Arc::new(|| ComposedInference {
        inference: Box::new(TwoStepInference { used: false.into() }),
        identity: Some(ModelIdentity::new(FIXTURE_IDENTITY.0, FIXTURE_IDENTITY.1)),
    })
}

/// Mock worker: answers `echo` calls with a diff-carrying output so
/// the SPA's hunk-review surface gets real Diff events.
pub(crate) async fn spawn_worker(state: &Arc<DaemonState>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    state.worker_router.register(
        "web-e2e-worker",
        vec![ToolGrant {
            tool: "echo".into(),
        }],
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

/// Fixture data the todo-20 dashboard surfaces consume: one confirmed
/// scheduled-run row for the history view and a pair of launcher
/// children retained in the ring so the workers panel is populated
/// straight from the attach snapshot. Seeded into both fixture
/// sessions — the drill scenario runs its turn on `e2e-drill` so the
/// scripted stack still has its echo step (the engine runs no
/// inference after a completed task).
pub(crate) fn seed_fixtures(state: &Arc<DaemonState>) {
    for sid in ["e2e", "e2e-drill"] {
        let _ = RunHistory::open(&state.config.data_dir).and_then(|history| {
            history.append(&RunRecord {
                automation_id: format!("{sid}-automation"),
                session_id: sid.into(),
                fired_at_minute: 0,
                outcome: "task_completed".into(),
                verdict: Some("confirmed".into()),
                network: NetworkPolicy::Online,
            })
        });
        let host = state.session(sid);
        host.publish(SessionEvent::ChildStarted {
            child: format!("{sid}-child-quick"),
            task: "quick".into(),
            role: "worker".into(),
            model: "claude-haiku-4-5".into(),
        });
        host.publish(SessionEvent::ChildProgress {
            child: format!("{sid}-child-quick"),
            phase: "file_touched".into(),
            detail: "src/demo.rs".into(),
        });
        host.publish(SessionEvent::ChildStarted {
            child: format!("{sid}-child-deep"),
            task: "deep".into(),
            role: "worker".into(),
            model: FIXTURE_IDENTITY.1.into(),
        });
        host.publish(SessionEvent::ChildFinished {
            child: format!("{sid}-child-deep"),
            task: "deep".into(),
            state: "success".into(),
            detail: "fixture complete".into(),
        });
        host.publish(SessionEvent::TeamBudget {
            spawned: 2,
            tokens_spent: 512,
            fan_out_cap: 4,
            token_budget: 100_000,
        });
    }
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
    seed_fixtures(&state);
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
