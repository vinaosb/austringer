//! Todo 3 (ecosystem alignment) acceptance: the six CORE_TOOLS served
//! through the REAL per-session tool registry. Sessions are created by
//! the daemon's own session factory (`DaemonState::session`) with the
//! executor factory injected exactly the way `bin/austringer-daemon.rs`
//! injects it; the inference is a scripted fake (hermetic: zero
//! network, zero keys), and every assertion rides the recorded event
//! stream.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::ContentBlock;

use austringer_daemon::attach::HostCommand;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::mediation::{MediationPolicy, PermissionMediator};
use austringer_daemon::protocol::{Answer, ServerMsg, SessionEvent};
use austringer_daemon::server::DaemonState;
use austringer_daemon::tools::CoreExecutors;

type Factory = Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>;

struct ScriptedInference {
    steps: std::sync::Mutex<std::collections::VecDeque<AssistantTurn>>,
}

impl ScriptedInference {
    fn from_steps(steps: Vec<AssistantTurn>) -> Self {
        Self {
            steps: std::sync::Mutex::new(steps.into()),
        }
    }
}

#[async_trait]
impl Inference for ScriptedInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let mut steps = match self.steps.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        steps
            .pop_front()
            .ok_or_else(|| InferenceError::Failure("script exhausted".into()))
    }
}

fn text_turn(text: &str, calls: Vec<PendingCall>) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: calls,
        usage: None,
    }
}

fn completion_turn() -> AssistantTurn {
    text_turn(
        "done",
        vec![PendingCall {
            call_id: "done".into(),
            tool: TASK_COMPLETED_TOOL.to_owned(),
            arguments: serde_json::json!({}),
        }],
    )
}

fn call(id: &str, tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: id.to_owned(),
        tool: tool.to_owned(),
        arguments,
    }
}

/// Collect the live event lane until the turn finishes.
async fn read_live_until_finished(mut live: broadcast::Receiver<ServerMsg>) -> Vec<String> {
    let mut frames = Vec::new();
    loop {
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => {
                let finished = matches!(frame.event, SessionEvent::TurnFinished { .. });
                if let Ok(encoded) = serde_json::to_string(&ServerMsg::Event { frame }) {
                    frames.push(encoded);
                }
                if finished {
                    return frames;
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return frames,
        }
    }
}

/// Answers every permission ask with AllowOnce (file-tool turns issue
/// one ask per call).
fn spawn_answerer(mediator: Arc<PermissionMediator>, client: &str) -> tokio::task::JoinHandle<()> {
    let client = client.to_owned();
    let mut watch = mediator.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = watch.recv().await {
            if let ServerMsg::PermissionAsk { ask } = msg {
                let _ = mediator.answer(ask.ask_id, &client, Answer::AllowOnce);
            }
        }
    })
}

/// The binary's startup wiring, mirrored: the executor bundle is
/// constructed once and the factory injected into the state.
fn inject_core_executors(mut state: DaemonState) -> Arc<DaemonState> {
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    Arc::new(state)
}

/// A daemon state over the given data dir with scripted inference
/// (fresh script per session) and the in-process CORE_TOOLS executors.
fn bound_state(data_dir: &Path, factory: Factory) -> Arc<DaemonState> {
    let state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("bound-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    inject_core_executors(state)
}

/// One scripted session turn: submit, answer asks, collect the event
/// lane until the turn finishes.
async fn drive_turn(state: &DaemonState, sid: &str) -> Vec<String> {
    let host = state.session(sid);
    let attach = host.attach("tui", None).expect("attach");
    let answerer = spawn_answerer(host.mediator(), "tui");
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    let collected = tokio::time::timeout(Duration::from_secs(90), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    answerer.abort();
    collected
}

fn session_dir_of(data_dir: &Path, sid: &str) -> PathBuf {
    data_dir.join("sessions").join(sid)
}

/// Fixture file inside the session dir (created before the session).
fn fixture_in(data_dir: &Path, sid: &str, name: &str, contents: &str) -> PathBuf {
    let path = session_dir_of(data_dir, sid).join(name);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("session dir");
    std::fs::write(&path, contents).expect("fixture");
    path
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

// ---------------------------------------------------------------------------
// File tools through the real registry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executor_bound_view_window_returns_real_result() {
    const SID: &str = "s-bound-view";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixture_in(dir.path(), SID, "fixture.txt", "alpha\nbeta\ngamma\n");
    let args_path = path_string(&fixture);
    let script: Factory = Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "reading",
                vec![call(
                    "t1",
                    "view_window",
                    serde_json::json!({ "path": args_path, "start_line": 1 }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("fixture.txt (lines 1-3 of 3)"),
        "view_window must return the real window view:\n{joined}"
    );
    assert!(
        joined.contains("3 | gamma"),
        "view_window must show the windowed body:\n{joined}"
    );
    assert!(
        !joined.contains("no executor registered"),
        "view_window must not hit NoExecutor:\n{joined}"
    );
}

#[tokio::test]
async fn executor_bound_edit_lines_edits_fixture_file() {
    const SID: &str = "s-bound-edit";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixture_in(dir.path(), SID, "fixture.txt", "alpha\nbeta\ngamma\n");
    let args_path = path_string(&fixture);
    let script: Factory = Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "editing",
                vec![call(
                    "t1",
                    "edit_lines",
                    serde_json::json!({
                        "path": args_path,
                        "edits": [{ "line": 2, "old_text": "beta", "new_text": "BETA EDITED" }]
                    }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("BETA EDITED"),
        "edit_lines must return the changed-region fringe:\n{joined}"
    );
    assert!(
        !joined.contains("no executor registered"),
        "edit_lines must not hit NoExecutor:\n{joined}"
    );
    let on_disk = std::fs::read_to_string(&fixture).expect("read back");
    assert!(
        on_disk.contains("BETA EDITED"),
        "the fixture file must be edited on disk: {on_disk}"
    );
}

// ---------------------------------------------------------------------------
// The remaining core tools through the same path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executor_bound_execute_command_runs_hermetic_command() {
    const SID: &str = "s-bound-exec";
    let dir = tempfile::tempdir().expect("tempdir");
    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "running",
                vec![call(
                    "t1",
                    "execute_command",
                    serde_json::json!({
                        "command": "echo austringer-exec-bound-78f2",
                        "timeout_ms": 30000
                    }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("austringer-exec-bound-78f2"),
        "execute_command must return the command's windowed output:\n{joined}"
    );
    assert!(
        joined.contains("exit code 0"),
        "a successful echo must report its exit code:\n{joined}"
    );
    assert!(
        !joined.contains("no executor registered"),
        "execute_command must not hit NoExecutor:\n{joined}"
    );
}

#[tokio::test]
async fn executor_bound_lsp_query_is_bound_not_no_executor() {
    const SID: &str = "s-bound-lsp";
    let dir = tempfile::tempdir().expect("tempdir");
    // Hermetic by construction: `.zzz` routes to no language, so the
    // router answers its typed NoServer error without spawning a real
    // server (real-server spawn is the user PATH's call and stays out
    // of hermetic tests — see .omo/notepads todo-3 entry).
    let fixture = fixture_in(dir.path(), SID, "nav.zzz", "opaque\n");
    let args_path = path_string(&fixture);
    let script: Factory = Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "navigating",
                vec![call(
                    "t1",
                    "lsp_query",
                    serde_json::json!({
                        "kind": "hover", "file": args_path, "line": 1, "character": 0
                    }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    // The executor is BOUND: the answer is the LSP router's typed
    // NoServer miss for an unroutable extension, carried through the
    // registry's ExecutionFailed rendering.
    assert!(
        joined.contains("no language server configured for .zzz files"),
        "lsp_query must answer with the router's typed error:\n{joined}"
    );
    assert!(
        !joined.contains("no executor registered"),
        "lsp_query must be bound to the LSP router:\n{joined}"
    );
    assert!(
        !joined.contains("remote worker"),
        "lsp_query must not fall through to the worker executor:\n{joined}"
    );
}

#[tokio::test]
async fn executor_bound_tool_search_hydrates_core_schema() {
    const SID: &str = "s-bound-search";
    let dir = tempfile::tempdir().expect("tempdir");
    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "searching",
                vec![call(
                    "t1",
                    "tool_search",
                    serde_json::json!({ "query": "view_window" }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("hydrated tool schemas (1)"),
        "tool_search is served internally by the registry:\n{joined}"
    );
    assert!(
        joined.contains("Read a bounded window"),
        "the hydrated schema must be the day-one view_window contract:\n{joined}"
    );
}

#[tokio::test]
async fn executor_bound_task_completed_is_loop_intercepted() {
    const SID: &str = "s-bound-complete";
    let dir = tempfile::tempdir().expect("tempdir");
    let script: Factory =
        Arc::new(|| Box::new(ScriptedInference::from_steps(vec![completion_turn()])));
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("task completed"),
        "task_completed is answered by the loop, not an executor:\n{joined}"
    );
    assert!(
        joined.contains("\"outcome\":\"task_completed\""),
        "the turn must complete through the reserved path:\n{joined}"
    );
}

// ---------------------------------------------------------------------------
// Failure paths (typed, preserved)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executor_bound_unknown_tool_returns_typed_unknown_error() {
    const SID: &str = "s-bound-unknown";
    let dir = tempfile::tempdir().expect("tempdir");
    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "typo",
                vec![call("t1", "view_windw", serde_json::json!({ "path": "x" }))],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    let frames = drive_turn(&state, SID).await;
    let joined = frames.join("\n");
    // The result text is JSON-encoded inside the event frame, so the
    // typed error's quotes arrive escaped.
    assert!(
        joined.contains("unknown tool \\\"view_windw\\\""),
        "an unregistered tool name must get the typed unknown-tool error:\n{joined}"
    );
    assert!(
        joined.contains("closest matches"),
        "the typed error must steer the model with suggestions:\n{joined}"
    );
}
