//! Shared harness for TUI integration tests: a real daemon on an
//! ephemeral loopback port (scripted two-step inference + mock
//! worker), the client task wired to an [`App`], event draining with
//! deadlines, and TestBackend screen extraction for snapshots.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use harness_core::agent_loop::inference::{InferenceRequest, PendingCall, TASK_COMPLETED_TOOL};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::ContentBlock;
use harness_daemon::config::DaemonConfig;
use harness_daemon::server::{DaemonState, build_router};
use harness_tui::app::{App, UiAction, UiEvent};
use harness_tui::client::{self, ClientCmd, ClientEvent};

pub struct Spawned {
    pub base: String,
    pub token: String,
    pub state: Arc<DaemonState>,
}

pub struct OneShotToolInference {
    used: std::sync::atomic::AtomicBool,
}

impl OneShotToolInference {
    #[must_use]
    pub fn new() -> Self {
        Self {
            used: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for OneShotToolInference {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Inference for OneShotToolInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, harness_core::agent_loop::InferenceError> {
        let first = !self.used.swap(true, std::sync::atomic::Ordering::SeqCst);
        Ok(if first {
            AssistantTurn {
                blocks: vec![ContentBlock::Text {
                    text: "working on it".into(),
                }],
                tool_calls: vec![PendingCall {
                    call_id: "t1".into(),
                    tool: "echo".into(),
                    arguments: serde_json::json!({"q": 1}),
                }],
                usage: None,
            }
        } else {
            AssistantTurn {
                blocks: vec![ContentBlock::Text {
                    text: "all done".into(),
                }],
                tool_calls: vec![PendingCall {
                    call_id: "done".into(),
                    tool: TASK_COMPLETED_TOOL.to_owned(),
                    arguments: serde_json::json!({}),
                }],
                usage: None,
            }
        })
    }
}

/// Real daemon on 127.0.0.1:0 with a mock echo worker registered, so
/// submitted turns complete end to end.
pub async fn spawn_daemon() -> Result<Spawned, String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("e2e-token".into()),
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(OneShotToolInference::new()));
    let state = Arc::new(DaemonState::new(config, factory).map_err(|e| format!("state: {e}"))?);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("bind: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("addr: {e}"))?
        .port();
    let (worker_tx, mut worker_rx) = tokio::sync::mpsc::channel(4);
    state.worker_router.register(
        "w-tui-test",
        vec![harness_daemon::worker::ToolGrant {
            tool: "echo".into(),
        }],
        worker_tx,
    );
    let router = std::sync::Arc::clone(&state.worker_router);
    tokio::spawn(async move {
        while let Some(call) = worker_rx.recv().await {
            if let harness_daemon::worker::DaemonToWorker::ToolCall { call_id, .. } = call {
                router.deliver(&call_id, true, serde_json::json!("worker output"));
            }
        }
    });
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

/// Client task bound to the current tokio runtime, with the UI-side
/// channel handles the tests drive the app through.
pub struct Wired {
    pub events: std::sync::mpsc::Receiver<ClientEvent>,
    pub cmd: tokio::sync::mpsc::Sender<ClientCmd>,
}

pub fn wire(base: &str, session: &str, token: &str) -> Wired {
    let (ev_tx, ev_rx) = std::sync::mpsc::channel::<ClientEvent>();
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<ClientCmd>(16);
    let base = base.to_owned();
    let session = session.to_owned();
    let token = token.to_owned();
    tokio::spawn(async move {
        client::run(&base, &session, &token, "tui-test", ev_tx, cmd_rx).await;
    });
    Wired {
        events: ev_rx,
        cmd: cmd_tx,
    }
}

/// Step every pending client event into `app` (non-blocking).
pub fn pump(app: &mut App, wired: &Wired) -> Vec<UiAction> {
    let mut actions = Vec::new();
    while let Ok(event) = wired.events.try_recv() {
        actions.extend(app.step(UiEvent::from(event)));
    }
    actions
}

/// Step client events into `app` until `predicate` holds or the
/// deadline passes. The 20 ms recv parks the thread — this is
/// event-driven waiting, not CPU polling.
pub fn wait_until(
    app: &mut App,
    wired: &Wired,
    predicate: impl Fn(&App) -> bool,
    budget: Duration,
) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if predicate(app) {
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        let slice = deadline - now;
        match wired
            .events
            .recv_timeout(slice.min(Duration::from_millis(20)))
        {
            Ok(event) => {
                let _ = app.step(UiEvent::from(event));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return predicate(app),
        }
    }
}

/// Render `app` through a TestBackend of `width`x`height` and return
/// the screen as trimmed text lines joined by newlines.
pub fn screen(app: &App, width: u16, height: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = match ratatui::Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => return format!("terminal error: {error}"),
    };
    match terminal.draw(|frame| harness_tui::render::render(frame, app)) {
        Ok(_) => {}
        Err(error) => return format!("draw error: {error}"),
    }
    let buffer = terminal.backend().buffer();
    let mut rows = Vec::new();
    for y in 0..height {
        let row: String = (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect();
        rows.push(row.trim_end().to_owned());
    }
    rows.join("\n")
}
