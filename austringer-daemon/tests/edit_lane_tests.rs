//! The staged edit lane (todo 16 + todo 21 join) through the REAL
//! surfaces: (a) a session-driven `edit_lines` call publishing
//! `SessionEvent::Diff` on the owning session's event stream, and
//! lane-level proofs of (b) disk-equals-staged-application, (c) typed
//! mid-air drift rejection with the file left intact, and (d) the
//! syntax gate rejecting a breaking edit before any write. Hermetic:
//! scripted inference, tempdirs, zero network.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

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
use austringer_daemon::tools::edit_lane::{self, LaneError};
use austringer_tools::edit::LineEdit;

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

fn call(id: &str, tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: id.to_owned(),
        tool: tool.to_owned(),
        arguments,
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

async fn read_live_until_finished(mut live: tokio::sync::broadcast::Receiver<ServerMsg>) {
    loop {
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => {
                if matches!(frame.event, SessionEvent::TurnFinished { .. }) {
                    return;
                }
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

fn bound_state(data_dir: &Path, factory: Factory) -> Arc<DaemonState> {
    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("edit-lane-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    Arc::new(state)
}

fn fixture(path: &Path, contents: &str) -> PathBuf {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("fixture parent");
    }
    std::fs::write(path, contents).expect("fixture write");
    path.to_path_buf()
}

fn hex_of(contents: &str) -> String {
    blake3::hash(contents.as_bytes()).to_hex().to_string()
}

fn line_edit(line: usize, old_text: &str, new_text: &str) -> LineEdit {
    LineEdit {
        line,
        old_text: old_text.to_owned(),
        new_text: new_text.to_owned(),
        hash: None,
    }
}

// ---------------------------------------------------------------------------
// (a) a session edit emits SessionEvent::Diff and writes the staged bytes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edit_lane_session_edit_emits_diff_event_and_writes_staged_bytes() {
    const SID: &str = "s-edit-lane-diff";
    let dir = tempfile::tempdir().expect("tempdir");
    let before = "fn greet(name: &str) -> String {\n    format!(\"hello {name}\")\n}\n";
    let after = "fn greet(name: &str) -> String {\n    format!(\"hi {name}\")\n}\n";
    let target = fixture(&dir.path().join("greet.rs"), before);
    let args_path = target.to_string_lossy().replace('\\', "/");
    let script: Factory = Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "editing",
                vec![call(
                    "t1",
                    "edit_lines",
                    serde_json::json!({
                        "path": args_path,
                        "edits": [{
                            "line": 2,
                            "old_text": "    format!(\"hello {name}\")",
                            "new_text": "    format!(\"hi {name}\")"
                        }]
                    }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);
    let host = state.session(SID);
    let attach = host.attach("tui", None).expect("attach");
    let answerer = spawn_answerer(host.mediator(), "tui");
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    tokio::time::timeout(Duration::from_secs(90), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    answerer.abort();

    // The pump task publishes asynchronously; poll the hub's ring
    // (authoritative) until the diff lands.
    let mut diff = None;
    for _ in 0..100 {
        if let Some(frame) = host
            .snapshot()
            .into_iter()
            .find(|frame| matches!(frame.event, SessionEvent::Diff { .. }))
        {
            diff = Some(frame.event);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    match diff.expect("session edit must emit a SessionEvent::Diff") {
        SessionEvent::Diff {
            path,
            before_hash,
            after_hash,
            hunks,
        } => {
            assert_eq!(path, target.to_string_lossy().replace('\\', "/"));
            assert_eq!(before_hash, hex_of(before));
            assert_eq!(after_hash, hex_of(after));
            assert_eq!(hunks.len(), 1, "one replacement, one hunk: {hunks:?}");
            assert!(
                hunks[0].starts_with("@@ -2 +2 @@ fn greet"),
                "the hunk must be unified-diff shaped and entity-labeled: {}",
                hunks[0]
            );
            assert!(
                hunks[0].contains("-    format!(\"hello {name}\")")
                    && hunks[0].contains("+    format!(\"hi {name}\")"),
                "the hunk must carry the removed and added lines: {}",
                hunks[0]
            );
        }
        other => panic!("expected a diff event, got {other:?}"),
    }

    let on_disk = std::fs::read_to_string(&target).expect("read back");
    assert_eq!(on_disk, after, "disk must equal the staged application");
}

// ---------------------------------------------------------------------------
// (b) the lane writes exactly the staged application
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edit_lane_commit_writes_staged_application() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = fixture(&dir.path().join("plain.txt"), "alpha\nbeta\ngamma\ndelta\n");
    let staged = edit_lane::staged_edit(
        &target,
        &[line_edit(2, "beta", "BETA"), line_edit(4, "delta", "DELTA")],
    )
    .expect("staged");
    assert_eq!(staged.before_hash, hex_of("alpha\nbeta\ngamma\ndelta\n"));
    assert_eq!(staged.after_hash, hex_of("alpha\nBETA\ngamma\nDELTA\n"));
    assert_eq!(
        staged.hunks,
        vec![
            "@@ -2 +2 @@\n-beta\n+BETA".to_owned(),
            "@@ -4 +4 @@\n-delta\n+DELTA".to_owned(),
        ],
        "one hunk per replacement, parseable @@ headers"
    );
    let committed = edit_lane::commit_staged(staged).expect("commit");
    assert_eq!(committed.after_hash, hex_of("alpha\nBETA\ngamma\nDELTA\n"));
    let on_disk = std::fs::read_to_string(&target).expect("read back");
    assert_eq!(on_disk, "alpha\nBETA\ngamma\nDELTA\n");
}

// ---------------------------------------------------------------------------
// (c) mid-air mutation between staging and commit is a typed drift
//     failure and the file is left exactly as the mutator left it
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edit_lane_mid_air_mutation_is_typed_drift_and_file_intact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = fixture(&dir.path().join("raced.txt"), "one\ntwo\nthree\n");
    let staged = edit_lane::staged_edit(&target, &[line_edit(2, "two", "TWO")])
        .expect("staged against the original bytes");
    std::fs::write(&target, "one\nMOVED\nthree\n").expect("mid-air mutation");
    let error = edit_lane::commit_staged(staged)
        .expect_err("a drifted commit must fail typed")
        .to_string();
    assert!(
        error.contains("content hash mismatch"),
        "the drift failure must carry the VFS typed text: {error}"
    );
    let on_disk = std::fs::read_to_string(&target).expect("read back");
    assert_eq!(
        on_disk, "one\nMOVED\nthree\n",
        "a rejected commit must not touch or corrupt the file"
    );
}

// ---------------------------------------------------------------------------
// (d) a syntax-breaking edit is rejected by the gate before any write
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edit_lane_syntax_breaking_edit_rejected_file_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let before = "fn main() {\n    println!(\"hi\");\n}\n";
    let target = fixture(&dir.path().join("broken.rs"), before);
    let error = edit_lane::staged_edit(
        &target,
        &[line_edit(2, "    println!(\"hi\");", "    println!(\"hi\"")],
    )
    .expect_err("a syntax-breaking edit must be rejected");
    assert!(
        matches!(
            error,
            LaneError::Tools(austringer_tools::ToolsError::SyntaxGate { .. })
        ),
        "the gate rejection must be the typed ToolsError::SyntaxGate: {error}"
    );
    assert!(
        error.to_string().contains("rejected by syntax gate"),
        "the model-facing text must name the gate: {error}"
    );
    let on_disk = std::fs::read_to_string(&target).expect("read back");
    assert_eq!(on_disk, before, "a gate rejection must never write");
}
