//! Todo 16 (ecosystem alignment) acceptance: the interactive plan
//! mode. The golden-transcript test at the top was recorded at the
//! PRE-task-16 HEAD (the plan's ordering mandate): it proves plan mode
//! is OFF by default by byte-comparing a mode-off session transcript
//! against the committed fixture. The remaining tests pin the plan
//! gate, the mediated approve/edit/reject flow, and the TUI wiring.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use harness_core::agent_loop::inference::{InferenceError, InferenceRequest, PendingCall};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::ContentBlock;

use harness_daemon::attach::HostCommand;
use harness_daemon::config::DaemonConfig;
use harness_daemon::mediation::{MediationPolicy, PermissionMediator};
use harness_daemon::plan;
use harness_daemon::protocol::{Answer, PlanDecision, ServerMsg, SessionEvent};
use harness_daemon::server::DaemonState;
use harness_daemon::tools::CoreExecutors;

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

/// Answers every permission ask with AllowOnce.
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

fn inject_core_executors(mut state: DaemonState) -> Arc<DaemonState> {
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    Arc::new(state)
}

fn plan_state(data_dir: &Path, factory: Factory) -> Arc<DaemonState> {
    let state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("plan-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    inject_core_executors(state)
}

fn session_dir_of(data_dir: &Path, sid: &str) -> PathBuf {
    data_dir.join("sessions").join(sid)
}

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
// Golden session transcript (recorded at pre-task-16 HEAD)
// ---------------------------------------------------------------------------

/// Where the committed golden fixture lives.
fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("task16_golden_session_transcript.json")
}

/// Drives the fixed two-turn scenario (an edit_lines turn answered by
/// an AllowOnce ask, then a plain text turn) and returns the session's
/// full frame snapshot as deterministic JSON. The tempdir root is
/// scrubbed to a placeholder so the bytes are machine-independent.
async fn drive_golden_scenario() -> String {
    const SID: &str = "s-golden16";
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
            text_turn("edited", Vec::new()),
            text_turn("second turn reply", Vec::new()),
        ]))
    });
    let state = plan_state(dir.path(), script);
    let host = state.session(SID);

    // Turn 1: the edit_lines call asks; client "gold" allows it once.
    let attach = host.attach("gold", None).expect("attach");
    let answerer = spawn_answerer(host.mediator(), "gold");
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    tokio::time::timeout(Duration::from_secs(90), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    answerer.abort();

    // Turn 2: plain reply, no calls.
    let attach2 = host.attach("gold-b", None).expect("attach 2");
    let reader2 = tokio::spawn(read_live_until_finished(attach2.events.live));
    host.handle_command(HostCommand::Submit("second".into()))
        .await
        .expect("submit 2");
    tokio::time::timeout(Duration::from_secs(90), reader2)
        .await
        .expect("turn 2 never finished")
        .expect("reader 2 failed");

    let mut transcript =
        serde_json::to_string_pretty(&host.snapshot()).expect("serialize snapshot");
    // Scrub the unique-per-run tempdir root (it appears inside tool
    // arguments and edit fringes) so the fixture bytes are portable.
    let root = path_string(dir.path());
    transcript = transcript.replace(&root, "<SESSION-ROOT>");
    transcript
}

/// Plan mode is OFF by default: a session driven with NO plan-mode
/// commands must produce a transcript byte-identical to the fixture
/// recorded at pre-task-16 HEAD. Re-record deliberately with
/// `HARNESS_UPDATE_GOLDEN=1` (never to make a diff pass silently).
#[tokio::test]
async fn golden_transcript_unchanged_with_plan_mode_off() {
    let transcript = drive_golden_scenario().await;
    let path = golden_path();
    if std::env::var_os("HARNESS_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("fixtures dir")).expect("fixtures dir");
        std::fs::write(&path, &transcript).expect("record golden fixture");
        return;
    }
    let golden = std::fs::read_to_string(&path)
        .expect("golden fixture must exist (record once with HARNESS_UPDATE_GOLDEN=1)");
    assert_eq!(
        transcript, golden,
        "plan mode OFF must leave the session transcript byte-identical \
         (a diff means the plan-mode wiring changed default-path behavior)"
    );
}

// ---------------------------------------------------------------------------
// Interactive plan mode (todo 16)
// ---------------------------------------------------------------------------

fn plan_state_with_timeout(
    data_dir: &Path,
    factory: Factory,
    ask_timeout: Duration,
) -> Arc<DaemonState> {
    let state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("plan-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout,
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    inject_core_executors(state)
}

fn edit_call(id: &str, args_path: &str) -> PendingCall {
    call(
        id,
        "edit_lines",
        serde_json::json!({
            "path": args_path,
            "edits": [{ "line": 2, "old_text": "beta", "new_text": "BETA EDITED" }]
        }),
    )
}

/// One turn with an edit_lines attempt followed by a plain reply.
fn edit_then_reply_script(args_path: String, reply: &'static str) -> Factory {
    Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn("planning", vec![edit_call("t1", &args_path)]),
            text_turn(reply, Vec::new()),
        ]))
    })
}

async fn drive_plan_turn(state: &DaemonState, sid: &str, prompt: &str) -> Vec<String> {
    let host = state.session(sid);
    let attach = host.attach("gold", None).expect("attach");
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit(prompt.to_owned()))
        .await
        .expect("submit");
    tokio::time::timeout(Duration::from_secs(90), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed")
}

/// Answers plan asks with `decision` (records every observed ask).
fn spawn_plan_answerer(
    mediator: Arc<PermissionMediator>,
    client: &str,
    decisions: Vec<PlanDecision>,
) -> tokio::task::JoinHandle<()> {
    let client = client.to_owned();
    let mut watch = mediator.subscribe();
    tokio::spawn(async move {
        let mut next = decisions.into_iter();
        while let Ok(msg) = watch.recv().await {
            if let ServerMsg::PlanAsk { ask } = msg
                && let Some(decision) = next.next()
            {
                let _ = mediator.answer_plan(ask.ask_id, &client, decision);
            }
        }
    })
}

/// Waits (bounded) until `predicate` holds over the session snapshot.
async fn await_snapshot(
    host: &Arc<harness_daemon::attach::SessionHost>,
    predicate: impl Fn(&[String]) -> bool,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        let frames: Vec<String> = host
            .snapshot()
            .iter()
            .filter_map(|f| serde_json::to_string(f).ok())
            .collect();
        if predicate(&frames) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition never observed on the snapshot within deadline");
}

#[tokio::test]
async fn plan_mode_blocks_edit_pre_approval_with_plan_id() {
    const SID: &str = "s-plan-block";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixture_in(dir.path(), SID, "fixture.txt", "alpha\nbeta\ngamma\n");
    let args_path = path_string(&fixture);
    let script = edit_then_reply_script(args_path, "plan drafted");
    let state = plan_state(dir.path(), script);
    let host = state.session(SID);

    plan::set_plan_mode(&host, true);
    let mut control = host.mediator().subscribe();
    let frames = drive_plan_turn(&state, SID, "edit the file").await;
    let joined = frames.join("\n");

    assert!(
        joined.contains("\"plan_blocked\""),
        "the blocked edit must surface the typed plan_blocked event:\n{joined}"
    );
    assert!(
        joined.contains("\"plan\":1") && joined.contains("edit_lines"),
        "the block event carries the pending plan id and the tool:\n{joined}"
    );
    assert!(
        joined.contains("plan 1"),
        "the tool-result block reason must name the pending plan id:\n{joined}"
    );
    let on_disk = std::fs::read_to_string(&fixture).expect("read back");
    assert!(
        !on_disk.contains("BETA EDITED"),
        "a write before approval must never touch the disk: {on_disk}"
    );

    let mut saw_plan_ask = false;
    let mut saw_permission_ask = false;
    while let Ok(msg) = control.try_recv() {
        match msg {
            ServerMsg::PlanAsk { ask } => {
                saw_plan_ask = true;
                assert_eq!(ask.plan_id, 1);
                assert_eq!(ask.revision, 1);
            }
            ServerMsg::PermissionAsk { .. } => saw_permission_ask = true,
            _ => {}
        }
    }
    assert!(saw_plan_ask, "the block must broadcast a plan approval ask");
    assert!(
        !saw_permission_ask,
        "the pre-approval block must fire the plan ask, never a permission ask"
    );
}

#[tokio::test]
async fn plan_approval_lets_subsequent_edit_proceed() {
    const SID: &str = "s-plan-approve";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixture_in(dir.path(), SID, "fixture.txt", "alpha\nbeta\ngamma\n");
    let args_path = path_string(&fixture);
    let script: Factory = {
        let args_path = args_path.clone();
        Arc::new(move || {
            Box::new(ScriptedInference::from_steps(vec![
                text_turn("planning", vec![edit_call("t1", &args_path)]),
                text_turn("plan drafted", Vec::new()),
                text_turn("executing", vec![edit_call("t2", &args_path)]),
                text_turn("done editing", Vec::new()),
            ]))
        })
    };
    let state = plan_state(dir.path(), script);
    let host = state.session(SID);

    plan::set_plan_mode(&host, true);
    let approver = spawn_plan_answerer(host.mediator(), "gold", vec![PlanDecision::Approve]);
    let permit_answerer = spawn_answerer(host.mediator(), "gold");

    let turn1 = drive_plan_turn(&state, SID, "plan then edit").await;
    assert!(
        turn1.join("\n").contains("\"plan_blocked\""),
        "turn 1 edit is pre-approval blocked"
    );

    await_snapshot(&host, |frames| {
        frames.iter().any(|f| f.contains("\"plan_approved\""))
    })
    .await;
    assert!(!host.plan_gate().is_active(), "approval flips plan mode off");

    let turn2 = drive_plan_turn(&state, SID, "now edit").await;
    let joined = format!("{}\n{}", turn1.join("\n"), turn2.join("\n"));
    assert!(
        joined.contains("BETA EDITED"),
        "after approval the edit must execute through the normal permission path:\n{joined}"
    );
    let on_disk = std::fs::read_to_string(&fixture).expect("read back");
    assert!(on_disk.contains("BETA EDITED"), "the edit must land on disk");

    approver.abort();
    permit_answerer.abort();
}

#[tokio::test]
async fn plan_edit_updates_artifact_and_re_gates() {
    const SID: &str = "s-plan-edit";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixture_in(dir.path(), SID, "fixture.txt", "alpha\nbeta\ngamma\n");
    let args_path = path_string(&fixture);
    let script: Factory = {
        let args_path = args_path.clone();
        Arc::new(move || {
            Box::new(ScriptedInference::from_steps(vec![
                text_turn("planning", vec![edit_call("t1", &args_path)]),
                text_turn("plan drafted", Vec::new()),
                text_turn("retry", vec![edit_call("t2", &args_path)]),
                text_turn("still gated", Vec::new()),
            ]))
        })
    };
    let state = plan_state(dir.path(), script);
    let host = state.session(SID);

    plan::set_plan_mode(&host, true);
    let mut control = host.mediator().subscribe();
    let editor =
        spawn_plan_answerer(host.mediator(), "gold", vec![PlanDecision::Edit {
            artifact: "# Revised plan\n- task one\n- files: src/x.rs".to_owned(),
        }]);

    let turn1 = drive_plan_turn(&state, SID, "plan").await;
    assert!(turn1.join("\n").contains("\"plan_blocked\""));

    await_snapshot(&host, |frames| {
        frames
            .iter()
            .any(|f| f.contains("\"plan_updated\"") && f.contains("\"revision\":2"))
    })
    .await;

    let turn2 = drive_plan_turn(&state, SID, "retry").await;
    let joined = format!("{}\n{}", turn1.join("\n"), turn2.join("\n"));
    assert!(
        joined.matches("\"plan_blocked\"").count() >= 2,
        "the edited plan must re-gate writes:\n{joined}"
    );

    let mut plan_asks = Vec::new();
    while let Ok(msg) = control.try_recv() {
        if let ServerMsg::PlanAsk { ask } = msg {
            plan_asks.push(ask);
        }
    }
    assert!(
        plan_asks.iter().any(|a| a.artifact.contains("Revised plan")),
        "a fresh ask must carry the edited artifact: {plan_asks:?}"
    );
    assert!(
        plan_asks.iter().all(|a| a.plan_id == 1),
        "edits keep the same plan id"
    );

    let on_disk = std::fs::read_to_string(&fixture).expect("read back");
    assert!(!on_disk.contains("BETA EDITED"), "still gated after the edit");
    assert!(host.plan_gate().is_active(), "edit keeps plan mode on");
    editor.abort();
}

#[tokio::test]
async fn plan_ask_timeout_does_not_approve() {
    const SID: &str = "s-plan-timeout";
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixture_in(dir.path(), SID, "fixture.txt", "alpha\nbeta\ngamma\n");
    let args_path = path_string(&fixture);
    let script: Factory = {
        let args_path = args_path.clone();
        Arc::new(move || {
            Box::new(ScriptedInference::from_steps(vec![
                text_turn("planning", vec![edit_call("t1", &args_path)]),
                text_turn("plan drafted", Vec::new()),
                text_turn("retry", vec![edit_call("t2", &args_path)]),
                text_turn("still gated", Vec::new()),
            ]))
        })
    };
    let state = plan_state_with_timeout(dir.path(), script, Duration::from_millis(150));
    let host = state.session(SID);

    plan::set_plan_mode(&host, true);
    let mut control = host.mediator().subscribe();
    let _ = drive_plan_turn(&state, SID, "plan").await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut timed_out = false;
    while tokio::time::Instant::now() < deadline {
        if let Ok(ServerMsg::PlanResolved { outcome, .. }) = control.try_recv() {
            if matches!(outcome, harness_daemon::protocol::PlanOutcome::Timeout) {
                timed_out = true;
                break;
            }
        } else {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    assert!(timed_out, "the unattended ask must resolve timeout, never approve");

    let turn2 = drive_plan_turn(&state, SID, "retry").await;
    let joined = turn2.join("\n");
    assert!(
        joined.contains("\"plan_blocked\""),
        "a timed-out plan must still gate the next write:\n{joined}"
    );
    assert!(
        !joined.contains("\"plan_approved\"") && !host.snapshot().is_empty(),
        "timeout never approves"
    );
    assert!(
        host.plan_gate().is_active(),
        "skip-permissions-style auto-approval must never happen on timeout"
    );
    let on_disk = std::fs::read_to_string(&fixture).expect("read back");
    assert!(!on_disk.contains("BETA EDITED"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_answer_over_http_is_typed_rejected_but_mode_flips_work() {
    use harness_daemon::server::build_router;

    let dir = tempfile::tempdir().expect("tempdir");
    let state = plan_state_with_timeout(
        dir.path(),
        Arc::new(|| {
            Box::new(ScriptedInference::from_steps(vec![text_turn(
                "idle",
                Vec::new(),
            )]))
        }),
        Duration::from_secs(30),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let app = build_router(Arc::clone(&state));
    tokio::spawn(async move {
        let serve = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        let _ = serve.await;
    });
    let base = format!("127.0.0.1:{port}");
    let client = reqwest::Client::new();

    // A plan answer from the identity-less HTTP route is refused typed
    // (the Antigravity v1.3.2 receipt: no surface may auto-approve a
    // plan; the mediated answer path needs a connection identity).
    let answer = serde_json::json!({
        "type": "plan_answer", "ask": 1, "decision": "approve",
    });
    let status = client
        .post(format!("http://{base}/s/s-plan-http/cmd"))
        .bearer_auth("plan-token")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(answer.to_string())
        .send()
        .await
        .expect("http")
        .status()
        .as_u16();
    assert_eq!(status, 400, "plan answers over HTTP must be refused typed");

    // Mode flips are not decisions: the same route accepts PlanMode.
    let mode_on = serde_json::json!({"type": "plan_mode", "on": true});
    let status = client
        .post(format!("http://{base}/s/s-plan-http/cmd"))
        .bearer_auth("plan-token")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(mode_on.to_string())
        .send()
        .await
        .expect("http")
        .status()
        .as_u16();
    assert_eq!(status, 202, "PlanMode is accepted on the command route");
    let host = state.session("s-plan-http");
    assert!(host.plan_gate().is_active(), "the mode flip took effect");
}
