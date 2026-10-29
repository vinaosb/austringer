//! Janitor vs REAL engine state: `janitor_tests` hand-fakes the
//! per-session dirs the janitor cleans; these tests prove the
//! orphan-cleanup contract against genuine engine output. Real
//! `DaemonState` turns create `{data}/journal/{sid}/{turn}.jsonl`,
//! snapshot checkpoints under `{data}/sessions/{sid}/`, and the store
//! row plus its `{data}/{sid}/` dir — then: a young session survives a
//! pass fully functional, a GC'd session loses every engine artifact
//! together with its row, and a pinned session (host evicted, so the
//! PIN is the only guard) keeps everything under an aggressive policy.
//!
//! Scripted inference uses PLAIN text turns (the eviction_tests
//! fixture): `RunOutcome::TurnComplete` keeps the loop recording, so
//! every submit is a productive turn. A task-completed turn would
//! terminally `Success` the loop and later submits would record
//! nothing.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{InferenceError, InferenceRequest};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::ContentBlock;
use austringer_daemon::attach::HostCommand;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::eviction;
use austringer_daemon::janitor;
use austringer_daemon::protocol::{Answer, ServerMsg, SessionEvent};
use austringer_daemon::server::DaemonState;
use austringer_daemon::tools::CoreExecutors;
use austringer_sessions::{RetentionPolicy, SessionError, SessionMeta, SessionStore};

type Factory = Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>;

/// One text-only assistant step per submit; no tool calls, no asks.
struct PlainTurn;

#[async_trait]
impl Inference for PlainTurn {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "done".into(),
            }],
            tool_calls: Vec::new(),
            usage: None,
        })
    }
}

/// Young + under budget: nothing a pass does can touch a session
/// created seconds ago (wall clock, no injection needed).
fn young() -> RetentionPolicy {
    RetentionPolicy {
        archive_after_secs: Some(3600),
        max_store_bytes: None,
        gc_archived_after_secs: Some(3600),
    }
}

/// Aggressive-but-deterministic (the janitor_tests policy): age tier
/// archives everything, GC tier removes every archive.
fn aggressive() -> RetentionPolicy {
    RetentionPolicy {
        archive_after_secs: Some(0),
        max_store_bytes: None,
        gc_archived_after_secs: Some(0),
    }
}

fn state_with(data_dir: &Path, retention: RetentionPolicy) -> Arc<DaemonState> {
    let factory: Factory = Arc::new(|| Box::new(PlainTurn));
    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            retention,
            ..DaemonConfig::default()
        },
        factory,
    )
    .expect("daemon state");
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    Arc::new(state)
}

async fn wait_turns(mut live: broadcast::Receiver<ServerMsg>, wanted: usize) {
    let mut finished = 0usize;
    while finished < wanted {
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => {
                finished += matches!(frame.event, SessionEvent::TurnFinished { .. }) as usize;
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => panic!("hub closed before {wanted} turns"),
        }
    }
}

/// Drive `count` REAL turns on one session (attach + submit + wait for
/// TurnFinished), then detach so the host is evictable.
async fn drive_turns(state: &DaemonState, sid: &str, client: &str, count: usize) {
    let host = state.session(sid);
    let attach = host.attach(client, None).expect("attach");
    let answerer = {
        let mediator = host.mediator();
        let client = client.to_owned();
        tokio::spawn(async move {
            let mut watch = mediator.subscribe();
            while let Ok(ServerMsg::PermissionAsk { ask }) = watch.recv().await {
                let _ = mediator.answer(ask.ask_id, &client, Answer::AllowOnce);
            }
        })
    };
    let reader = tokio::spawn(wait_turns(attach.events.live, count));
    for _ in 0..count {
        host.handle_command(HostCommand::Submit("go".into()))
            .await
            .expect("submit");
    }
    tokio::time::timeout(Duration::from_secs(120), reader)
        .await
        .expect("turns never finished")
        .expect("reader failed");
    answerer.abort();
    host.detach(client);
}

/// Sweep the idle host away, so the janitor's live-host guard is NOT
/// what protects the session in the assertions that follow.
async fn evict_idle(state: &DaemonState, sid: &str) {
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(
        eviction::run_once(state, Duration::from_millis(1)),
        vec![sid.to_owned()]
    );
    assert!(
        state.try_session(sid).is_none(),
        "host must actually be evicted"
    );
}

fn journal_file(data: &Path, sid: &str, turn: u64) -> PathBuf {
    data.join("journal").join(sid).join(format!("{turn}.jsonl"))
}

fn meta_of(data: &Path, sid: &str) -> SessionMeta {
    SessionStore::open(data)
        .unwrap()
        .open_session(sid)
        .expect("session row")
        .meta()
        .clone()
}

/// Scenario A: two real turns build the genuine engine layout; a
/// young-and-under-budget pass touches none of it, and the session
/// keeps working (one more turn lands, transcript grows in place).
#[tokio::test]
async fn janitor_lifecycle_young_real_session_survives_pass_and_stays_functional() {
    const SID: &str = "jlc-young";
    let dir = tempfile::tempdir().unwrap();
    let state = state_with(dir.path(), young());

    drive_turns(&state, SID, "tui", 2).await;
    assert!(journal_file(dir.path(), SID, 1).is_file(), "turn 1 journal");
    assert!(journal_file(dir.path(), SID, 2).is_file(), "turn 2 journal");
    let engine_dir = dir.path().join("sessions").join(SID);
    let store_dir = dir.path().join(SID);
    let checkpoint = engine_dir.join("checkpoints").join("0000000001.json");
    assert!(engine_dir.is_dir(), "engine session dir");
    assert!(checkpoint.is_file(), "turn-completion checkpoint");
    assert!(
        store_dir.join("events.jsonl").is_file(),
        "store session dir"
    );
    let before = meta_of(dir.path(), SID);
    assert_eq!(before.turn_count, 2, "both turns persisted: {before:?}");
    assert!(before.event_count >= 4, "{before:?}");

    let pass = janitor::run_once(&state).unwrap();
    assert!(pass.report.archived.is_empty(), "{:?}", pass.report);
    assert!(pass.report.gc_removed.is_empty(), "{:?}", pass.report);
    assert!(journal_file(dir.path(), SID, 1).is_file());
    assert!(journal_file(dir.path(), SID, 2).is_file());
    assert!(engine_dir.is_dir());
    assert!(store_dir.is_dir());

    drive_turns(&state, SID, "post", 1).await;
    assert!(journal_file(dir.path(), SID, 3).is_file(), "turn 3 journal");
    let after = meta_of(dir.path(), SID);
    assert_eq!(after.turn_count, 3, "third turn persisted: {after:?}");
    assert!(
        after.last_seq > before.last_seq && after.event_count > before.event_count,
        "transcript must keep growing in place: {before:?} -> {after:?}"
    );
    let metas = SessionStore::open(dir.path()).unwrap().list().unwrap();
    assert_eq!(metas.len(), 1, "no row was touched");
    assert_eq!(metas[0].id, SID);
}

/// Scenario B: age + GC the session per the janitor contract — the
/// REAL engine dirs (`journal/{sid}`, `sessions/{sid}`, the store's
/// `{sid}`) and the store row leave together, and the cold archive is
/// deleted with them.
#[tokio::test]
async fn janitor_lifecycle_gc_removes_real_engine_dirs_with_the_store_row() {
    const SID: &str = "jlc-gc";
    let dir = tempfile::tempdir().unwrap();
    let state = state_with(dir.path(), aggressive());
    let cold = dir.path().join("archive").join(format!("{SID}.jsonl"));

    drive_turns(&state, SID, "tui", 2).await;
    assert!(journal_file(dir.path(), SID, 1).is_file());
    assert!(dir.path().join("sessions").join(SID).is_dir());
    assert!(dir.path().join(SID).is_dir());
    assert!(meta_of(dir.path(), SID).event_count > 0);
    evict_idle(&state, SID).await;

    let pass = janitor::run_once(&state).unwrap();
    assert_eq!(pass.report.archived.len(), 1, "{:?}", pass.report);
    assert!(cold.is_file(), "cold JSONL written");
    assert!(
        dir.path().join("sessions").join(SID).is_dir(),
        "archive is not deletion"
    );

    let pass = janitor::run_once(&state).unwrap();
    assert_eq!(
        pass.report.gc_removed,
        vec![SID.to_owned()],
        "{:?}",
        pass.report
    );
    assert!(
        !dir.path().join("journal").join(SID).exists(),
        "journal dir removed"
    );
    assert!(
        !dir.path().join("sessions").join(SID).exists(),
        "engine dir removed"
    );
    assert!(!dir.path().join(SID).exists(), "store session dir removed");
    assert!(!cold.exists(), "cold archive removed");
    let store = SessionStore::open(dir.path()).unwrap();
    assert!(
        matches!(store.open_session(SID), Err(SessionError::NotFound { .. })),
        "the store row must be gone with the dirs"
    );
    assert!(store.list().unwrap().is_empty());
}

/// Scenario C: pinned + host-evicted, so the PIN is the only guard —
/// aggressive passes delete nothing of the session's real dirs, its
/// store row, or its never-created cold archive.
#[tokio::test]
async fn janitor_lifecycle_pinned_real_session_survives_aggressive_passes() {
    const SID: &str = "jlc-pin";
    let dir = tempfile::tempdir().unwrap();
    let state = state_with(dir.path(), aggressive());
    let cold = dir.path().join("archive").join(format!("{SID}.jsonl"));

    drive_turns(&state, SID, "tui", 2).await;
    SessionStore::open(dir.path())
        .unwrap()
        .set_pinned(SID, true)
        .unwrap();
    evict_idle(&state, SID).await;

    let pass_one = janitor::run_once(&state).unwrap();
    assert_eq!(pass_one.report.pinned_skipped, vec![SID.to_owned()]);
    let pass_two = janitor::run_once(&state).unwrap();
    assert_eq!(pass_two.report.pinned_skipped, vec![SID.to_owned()]);
    assert!(
        pass_two.report.gc_removed.is_empty(),
        "{:?}",
        pass_two.report
    );

    assert!(
        journal_file(dir.path(), SID, 1).is_file(),
        "journal survives"
    );
    assert!(journal_file(dir.path(), SID, 2).is_file());
    assert!(
        dir.path().join("sessions").join(SID).is_dir(),
        "engine dir survives"
    );
    assert!(dir.path().join(SID).is_dir(), "store session dir survives");
    assert!(!cold.exists(), "a pinned session is never even archived");
    let meta = meta_of(dir.path(), SID);
    assert!(meta.pinned && meta.event_count > 0, "row intact: {meta:?}");
}
