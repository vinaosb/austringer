//! Wave-1 storage-hygiene acceptance: the daemon-side PTY runner must
//! spill oversized raw output under the SESSION directory
//! (`{data}/sessions/{sid}/spill` — the engine layout the registry's own
//! `SpillStore` already uses), never the global `%TEMP%\austringer-exec-spill`
//! default. The fixtures mirror `executor_bound_tests.rs`: the executor
//! bundle is constructed and injected exactly the way
//! `bin/austringer-daemon.rs` does it, inference is a scripted fake.

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

/// Answers every permission ask with AllowOnce (execute_command issues
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

/// The binary's startup wiring, mirrored.
fn inject_core_executors(mut state: DaemonState) -> Arc<DaemonState> {
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    Arc::new(state)
}

fn bound_state(data_dir: &Path, factory: Factory) -> Arc<DaemonState> {
    let state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("spill-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    inject_core_executors(state)
}

async fn drive_turn(state: &DaemonState, sid: &str) {
    let host = state.session(sid);
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
}

fn session_dir_of(data_dir: &Path, sid: &str) -> PathBuf {
    data_dir.join("sessions").join(sid)
}

/// The runner's raw-stream spill artifact: the raw PTY stream carries
/// the un-wrapped payload, so its longest unbroken `x` run (measured
/// ~4800 on the ConPTY pass-through) is orders of magnitude wider than
/// any windowed screen row in a registry-level result spill (which
/// wraps at the screen width and carries "exit code" framing).
fn is_raw_stream_spill(path: &Path, min_run: usize) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let mut best = 0usize;
    let mut current = 0usize;
    for byte in &bytes {
        if *byte == b'x' {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best >= min_run
}

#[tokio::test]
async fn executor_spill_lands_in_session_dir_not_global_temp() {
    const SID: &str = "s-spill-session-root";
    let dir = tempfile::tempdir().expect("tempdir");

    // The global default spill root must not pre-exist (a stale
    // dev-machine dir is %TEMP% garbage; clearing keeps the assertion
    // below meaningful on any machine).
    let global_default = std::env::temp_dir().join("austringer-exec-spill");
    if global_default.exists() {
        std::fs::remove_dir_all(&global_default).expect("clear stale global spill");
    }

    // 64 KiB of 'x': far over IoCap::DEFAULT's 4096-byte inline cap, so
    // the PTY runner's RawSpool must stream the raw output to its spill
    // store. The windowed model text stays far under the cap, so this
    // exercises exactly the runner's spill path.
    let script: Factory = Arc::new(|| {
        Box::new(ScriptedInference::from_steps(vec![
            text_turn(
                "flooding the pty",
                vec![call(
                    "t1",
                    "execute_command",
                    serde_json::json!({
                        "command": "powershell -NoProfile -Command \"'x' * 65536\"",
                        "timeout_ms": 60000
                    }),
                )],
            ),
            completion_turn(),
        ]))
    });
    let state = bound_state(dir.path(), script);

    drive_turn(&state, SID).await;

    // The raw stream must sit under the session dir's spill store.
    let session_spill = session_dir_of(dir.path(), SID).join("spill");
    let entries = std::fs::read_dir(&session_spill)
        .expect("session spill dir must exist")
        .map(|entry| entry.expect("entry").path())
        .collect::<Vec<_>>();
    assert!(
        entries.iter().any(|path| is_raw_stream_spill(path, 2000)),
        "the runner's raw-stream spill must land in {:?}; got {:?}",
        session_spill,
        entries
    );
    assert!(
        !global_default.exists(),
        "the global %TEMP% spill default must stay unused"
    );
}
