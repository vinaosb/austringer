//! Todo 4 (ecosystem alignment) acceptance: execute_command routed
//! through austringer-safety. Every spawn passes the per-command sandbox
//! gate — the egress guard blocks network-shaped commands under the
//! deny-all egress allowlist BEFORE anything spawns (hermetic: a
//! string-pattern verdict, zero sockets), and allowed runs carry the
//! requested-vs-enforced isolation report in the result (harness-rs
//! honesty: isolation is never claimed where it was not established).

use std::path::Path;
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

fn call(id: &str, tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: id.to_owned(),
        tool: tool.to_owned(),
        arguments,
    }
}

fn script_with_command(command: &str) -> Factory {
    let command = command.to_owned();
    Arc::new(move || {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "running",
                vec![call(
                    "t1",
                    "execute_command",
                    serde_json::json!({
                        "command": command,
                        "timeout_ms": 30000
                    }),
                )],
            ),
            text_turn(
                "done",
                vec![PendingCall {
                    call_id: "done".into(),
                    tool: TASK_COMPLETED_TOOL.to_owned(),
                    arguments: serde_json::json!({}),
                }],
            ),
        ]))
    })
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

/// The binary's startup wiring, mirrored (todo-3 pattern): the
/// executor bundle is constructed once and the factory injected.
fn safety_state(data_dir: &Path, factory: Factory) -> Arc<DaemonState> {
    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("safety-token".into()),
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

async fn drive_turn(state: &DaemonState, sid: &str) -> String {
    let host = state.session(sid);
    let attach = host.attach("tui", None).expect("attach");
    let answerer = {
        let mediator: Arc<PermissionMediator> = host.mediator();
        let client = "tui".to_owned();
        let mut watch = mediator.subscribe();
        tokio::spawn(async move {
            while let Ok(msg) = watch.recv().await {
                if let ServerMsg::PermissionAsk { ask } = msg {
                    let _ = mediator.answer(ask.ask_id, &client, Answer::AllowOnce);
                }
            }
        })
    };
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    let collected = tokio::time::timeout(Duration::from_secs(90), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    answerer.abort();
    collected.join("\n")
}

// ---------------------------------------------------------------------------
// Deny-egress blocks a curl-shaped command (fail-closed, pre-spawn)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executor_safety_deny_egress_blocks_curl_shaped_command() {
    const SID: &str = "s-safety-deny";
    let dir = tempfile::tempdir().expect("tempdir");
    // .invalid is RFC 2606 reserved (never resolvable) and the guard
    // blocks on the curl PROGRAM WORD before any spawn — no socket is
    // ever opened, on any platform, green or red.
    let state = safety_state(
        dir.path(),
        script_with_command("curl -s https://egress-blocked.invalid/lease"),
    );

    let joined = drive_turn(&state, SID).await;
    assert!(
        joined.contains("blocked by sandbox policy"),
        "a curl-shaped command under the deny-all egress policy must be \
         blocked with the typed reason:\n{joined}"
    );
    assert!(
        joined.contains("egress"),
        "the block must name the egress denial:\n{joined}"
    );
    assert!(
        joined.contains("egress-blocked.invalid"),
        "the block must name the denied destination host:\n{joined}"
    );
    assert!(
        joined.contains("requested:") && joined.contains("enforced:"),
        "the blocked result must state requested-vs-enforced:\n{joined}"
    );
    assert!(
        !joined.contains("no executor registered"),
        "the safety gate must sit on the bound executor, not bypass it:\n{joined}"
    );
}

// ---------------------------------------------------------------------------
// Allowed command runs with policy metadata in the result
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executor_safety_allowed_command_carries_policy_metadata() {
    const SID: &str = "s-safety-allow";
    let dir = tempfile::tempdir().expect("tempdir");
    let state = safety_state(
        dir.path(),
        script_with_command("echo austringer-safety-bound-4c1d"),
    );

    let joined = drive_turn(&state, SID).await;
    assert!(
        joined.contains("austringer-safety-bound-4c1d"),
        "the allowed command must still run through the PTY pipeline:\n{joined}"
    );
    assert!(
        joined.contains("exit code 0"),
        "a successful echo must report its exit code:\n{joined}"
    );
    assert!(
        joined.contains("requested:") && joined.contains("enforced:"),
        "the result must carry the requested-vs-enforced report:\n{joined}"
    );
    assert!(
        joined.contains("backend="),
        "the report must name the platform sandbox backend:\n{joined}"
    );
    assert!(
        joined.contains("gaps:"),
        "the report must name its isolation gaps honestly:\n{joined}"
    );
    assert!(
        !joined.contains("blocked by sandbox policy"),
        "an echo must not be blocked by the egress guard:\n{joined}"
    );
}
