//! Snapshot lifecycle acceptance: every completed turn records an
//! austringer-snapshots checkpoint (workspace snapshot + transcript
//! checkpoint + session-thread record), `/rewind` can list and restore
//! what turns record, policy GC bounds the count to the configured
//! keep-last, and a recording failure is a NON-FATAL noticed event
//! (never a turn killer). The fixtures mirror `executor_spill_tests.rs`
//! (the binary's DaemonState wiring + scripted inference); the tempdir
//! data root keeps the no-git backend, so the run is hermetic.

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
use austringer_daemon::rewind::run_rewind;
use austringer_daemon::server::DaemonState;
use austringer_daemon::tools::CoreExecutors;

use austringer_snapshots::SnapshotStore;
use austringer_snapshots::ids::{SessionId, Turn};

type Factory = Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>;

/// Always-completes inference: every submit runs exactly one assistant
/// step that calls the reserved task-completed tool, so N submits make
/// N finished turns with no permission asks.
struct AlwaysComplete;

#[async_trait]
impl Inference for AlwaysComplete {
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

/// Collect the live event lane until `wanted` TurnFinished frames
/// arrived (multi-turn driving: one attach, N submits, N finishes).
async fn read_live_until_turns(
    mut live: broadcast::Receiver<ServerMsg>,
    wanted: usize,
) -> Vec<String> {
    let mut frames = Vec::new();
    let mut finished = 0usize;
    while finished < wanted {
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => {
                if matches!(frame.event, SessionEvent::TurnFinished { .. }) {
                    finished += 1;
                }
                if let Ok(encoded) = serde_json::to_string(&ServerMsg::Event { frame }) {
                    frames.push(encoded);
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return frames,
        }
    }
    frames
}

/// Answers every permission ask with AllowOnce (kept from the spill
/// fixture pattern even though completion turns ask nothing).
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

fn bound_state(data_dir: &Path, factory: Factory, keep_last: usize) -> Arc<DaemonState> {
    let state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("snapshot-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            snapshot_keep_last: keep_last,
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    inject_core_executors(state)
}

/// Drive `count` scripted turns on one session and return every frame
/// seen between the first TurnStarted and the last TurnFinished.
async fn drive_turns(state: &DaemonState, sid: &str, count: usize) -> Vec<String> {
    let host = state.session(sid);
    let attach = host.attach("tui", None).expect("attach");
    let answerer = spawn_answerer(host.mediator(), "tui");
    let reader = tokio::spawn(read_live_until_turns(attach.events.live, count));
    for _ in 0..count {
        host.handle_command(HostCommand::Submit("go".into()))
            .await
            .expect("submit");
    }
    let frames = tokio::time::timeout(Duration::from_secs(120), reader)
        .await
        .expect("turns never finished")
        .expect("reader failed");
    answerer.abort();
    frames
}

fn session_dir_of(data_dir: &Path, sid: &str) -> PathBuf {
    data_dir.join("sessions").join(sid)
}

fn turns_of(data_dir: &Path, sid: &str) -> Vec<Turn> {
    SnapshotStore::open(&session_dir_of(data_dir, sid))
        .expect("open store")
        .turns_of(&SessionId::new(sid).expect("session id"))
        .expect("turns")
}

/// The transcript checkpoint sidecar of one turn (`checkpoints/<turn>.json`).
fn checkpoint_json(data_dir: &Path, sid: &str, turn: u32) -> serde_json::Value {
    let path = session_dir_of(data_dir, sid)
        .join("checkpoints")
        .join(format!("{turn:010}.json"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("checkpoint {path:?}: {e}"));
    serde_json::from_slice(&bytes).expect("checkpoint json")
}

/// Turn completion records a snapshot the rewind surface can list and
/// restore: two turns -> two restore points, the turn-1 checkpoint
/// carries a snapshot pointer and a through-seq, and `/rewind 2`
/// restores the turn-1 workspace state and truncates the persisted
/// log to the checkpoint's head.
#[tokio::test]
async fn turn_completion_records_snapshot_checkpoint_and_rewind_restores() {
    const SID: &str = "snap-e2e";
    let dir = tempfile::tempdir().expect("tempdir");
    let factory: Factory = Arc::new(|| Box::new(AlwaysComplete));
    let state = bound_state(dir.path(), factory, 10);

    let host = state.session(SID);
    let notes = session_dir_of(dir.path(), SID).join("notes.txt");
    std::fs::write(&notes, "v1").expect("seed notes");
    drop(host);

    let frames = drive_turns(&state, SID, 1).await;
    assert!(
        frames.iter().any(|f| f.contains("\"turn_started\"")),
        "turns ran: {frames:?}"
    );

    // One recorded restore point, listable exactly the way /rewind
    // lists them.
    assert_eq!(
        turns_of(dir.path(), SID),
        vec![Turn::new(1).unwrap()],
        "every completed turn must record a snapshot (no-git manifests)"
    );

    // The transcript checkpoint: a snapshot pointer plus a through-seq
    // (the rewind truncation cursor source).
    let checkpoint = checkpoint_json(dir.path(), SID, 1);
    assert_eq!(checkpoint["turn"], 1, "turn 1 sidecar: {checkpoint}");
    let through = checkpoint["through_seq"].as_u64().expect("through_seq");
    assert!(through >= 1, "cursor must cover the turn's events");
    assert!(
        checkpoint["meta"]["snapshot"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "checkpoint must name its snapshot target: {checkpoint}"
    );

    // Drift BETWEEN the two snapshots (the daemon's rewind skips files
    // modified after the LAST snapshot, so the change must land before
    // turn 2 records), then rewind two steps back to turn 1.
    std::fs::write(&notes, "v2").expect("drift notes");
    drive_turns(&state, SID, 1).await;
    assert_eq!(
        turns_of(dir.path(), SID),
        vec![Turn::new(1).unwrap(), Turn::new(2).unwrap()],
        "two turns, two restore points"
    );

    let host = state.session(SID);
    run_rewind(Arc::clone(&host), 2)
        .await
        .expect("rewind to the turn-1 snapshot");
    assert_eq!(
        std::fs::read_to_string(&notes).unwrap(),
        "v1",
        "rewind must restore the turn-1 workspace state"
    );

    // The persisted log truncated to the checkpoint head: nothing
    // past the turn-1 cursor survives.
    let store = austringer_sessions::SessionStore::open(&state.config.data_dir).unwrap();
    let meta = store.open_session(SID).expect("session row").meta().clone();
    assert_eq!(meta.last_seq, through, "log head must match the cursor");
    // The restore point itself is still listed (rewind never deletes).
    assert_eq!(turns_of(dir.path(), SID).len(), 2);
}

/// Policy GC bounds the recorded count: keep_last 2 with four turns
/// leaves exactly the newest two restore points, and the checkpoint
/// sidecars of GC'd turns are swept with them.
#[tokio::test]
async fn snapshot_gc_bounds_turns_to_configured_keep_last() {
    const SID: &str = "snap-gc";
    let dir = tempfile::tempdir().expect("tempdir");
    let factory: Factory = Arc::new(|| Box::new(AlwaysComplete));
    let state = bound_state(dir.path(), factory, 2);

    drive_turns(&state, SID, 4).await;

    assert_eq!(
        turns_of(dir.path(), SID),
        vec![Turn::new(3).unwrap(), Turn::new(4).unwrap()],
        "GC must bound the session to the configured keep_last"
    );
    let checkpoints = session_dir_of(dir.path(), SID).join("checkpoints");
    for kept in [3u32, 4] {
        assert!(
            checkpoints.join(format!("{kept:010}.json")).exists(),
            "kept turn {kept} must keep its checkpoint sidecar"
        );
    }
    for dropped in [1u32, 2] {
        assert!(
            !checkpoints.join(format!("{dropped:010}.json")).exists(),
            "GC'd turn {dropped} must not leave checkpoint residue"
        );
    }
}

/// A recording failure is fail-open WITH NOTICE: the turn still
/// finishes and a `snapshot` notification names the degradation —
/// never a silent drop, never a killed turn.
#[tokio::test]
async fn snapshot_recording_failure_is_non_fatal_noticed_turn() {
    const SID: &str = "snap-fail";
    let dir = tempfile::tempdir().expect("tempdir");
    let factory: Factory = Arc::new(|| Box::new(AlwaysComplete));
    let state = bound_state(dir.path(), factory, 10);

    let _host = state.session(SID);
    // Blocker: a FILE where the snapshot store wants its `.austringer`
    // root, so every write inside the recording pass fails typed.
    std::fs::write(session_dir_of(dir.path(), SID).join(".austringer"), b"").expect("blocker");

    let frames = drive_turns(&state, SID, 1).await;
    assert!(
        frames.iter().any(|f| f.contains("\"turn_finished\"")),
        "the turn must finish despite the recording failure: {frames:?}"
    );
    let notice = frames
        .iter()
        .find(|f| f.contains("\"kind\":\"snapshot\""))
        .unwrap_or_else(|| panic!("recording failure must be noticed: {frames:?}"));
    assert!(
        notice.contains("checkpoint"),
        "the notice must name what degraded: {notice}"
    );

    // Honest absence: nothing was recorded.
    assert!(
        turns_of(dir.path(), SID).is_empty(),
        "no snapshots may exist behind the blocker"
    );
    // The engine's own event log still landed in the sessions store
    // (the failure degraded only snapshots, not the turn).
    let store = austringer_sessions::SessionStore::open(&state.config.data_dir).unwrap();
    let meta = store.open_session(SID).expect("session row").meta().clone();
    assert!(meta.last_seq >= 1, "turn events must be persisted");
}
