//! Janitor (wave 2): the daemon-side caller that puts wave-1's
//! dormant sessions-retention engine to work — configured prune
//! tiers, orphan-dir cleanup driven ONLY by `gc_removed`, opt-in
//! vacuum, the all-`None` default-off posture, and the shutdown
//! readiness-file hygiene.

use std::sync::Arc;

use austringer_core::agent_loop::inference::{InferenceError, InferenceRequest};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::{ContentBlock, Event};
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::janitor;
use austringer_daemon::server::DaemonState;
use austringer_sessions::SessionError;
use austringer_sessions::SessionStore;
use austringer_sessions::lease::LeaseOwner;
use austringer_sessions::{RetentionPolicy, SessionMeta};

/// Inference that never runs: janitor tests never submit a turn.
struct IdleInference;

#[async_trait::async_trait]
impl Inference for IdleInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        unreachable!("janitor tests never submit turns")
    }
}

struct Fixture {
    state: Arc<DaemonState>,
    _dir: tempfile::TempDir,
}

fn state_with(tweak: impl FnOnce(DaemonConfig) -> DaemonConfig) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let config = tweak(DaemonConfig {
        data_dir: dir.path().to_owned(),
        ..DaemonConfig::default()
    });
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(IdleInference));
    let state = Arc::new(DaemonState::new(config, factory).unwrap());
    Fixture { state, _dir: dir }
}

/// Aggressive-but-deterministic policy with the wall clock: every
/// existing session is at least 0 seconds old, so the age tier
/// archives everything and the GC tier removes every archive.
fn aggressive() -> RetentionPolicy {
    RetentionPolicy {
        archive_after_secs: Some(0),
        max_store_bytes: None,
        gc_archived_after_secs: Some(0),
    }
}

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

/// Seed one session row with content in the daemon's store.
fn seed_session(data: &std::path::Path, id: &str) {
    let store = SessionStore::open(data).unwrap();
    store.create(id, "").unwrap();
    {
        let owner = LeaseOwner::new("janitor-fixture");
        let handle = store.open_session_rw(id, &owner).unwrap();
        handle.append(user(&format!("{id} content"))).unwrap();
    }
}

/// Fake the engine-side per-session dirs the janitor cleans:
/// `{data}/sessions/{id}/` and `{data}/journal/{id}/`.
fn fake_dirs(data: &std::path::Path, id: &str) {
    std::fs::create_dir_all(data.join("sessions").join(id)).unwrap();
    std::fs::create_dir_all(data.join("journal").join(id)).unwrap();
}

#[tokio::test]
async fn janitor_prunes_gcs_and_cleans_orphans_but_preserves_live_and_pinned() {
    let fixture = state_with(|c| DaemonConfig {
        retention: aggressive(),
        ..c
    });
    let state = &fixture.state;
    let data = &state.config.data_dir;

    seed_session(data, "old");
    seed_session(data, "pinned");
    seed_session(data, "live");
    {
        let store = SessionStore::open(data).unwrap();
        store.set_pinned("pinned", true).unwrap();
    }
    fake_dirs(data, "old");
    fake_dirs(data, "pinned");
    // A live host: the session factory materializes the engine dir,
    // the fixture materializes the journal dir and the store row.
    let _live = state.session("live");
    std::fs::create_dir_all(data.join("journal").join("live")).unwrap();

    // Pass one (age tier): idle sessions archive to cold JSONL; the
    // pinned one is exempt; nothing is deleted yet.
    let pass = janitor::run_once(state).unwrap();
    assert_eq!(pass.report.archived.len(), 2, "{:?}", pass.report);
    assert_eq!(pass.report.pinned_skipped, vec!["pinned".to_owned()]);
    let cold = data.join("archive").join("old.jsonl");
    assert!(cold.is_file(), "archived JSONL must be written");
    assert!(
        std::fs::read_to_string(&cold)
            .unwrap()
            .contains("old content"),
        "the cold file must carry the events"
    );
    assert!(data.join("archive").join("live.jsonl").is_file());
    assert!(
        data.join("sessions").join("old").is_dir(),
        "archive is not deletion: dirs survive pass one"
    );

    // Pass two (GC tier): tombstoned sessions leave the store and
    // ONLY gc_removed ids drive dir deletion.
    let pass = janitor::run_once(state).unwrap();
    let mut removed = pass.report.gc_removed.clone();
    removed.sort();
    assert_eq!(
        removed,
        vec!["live".to_owned(), "old".to_owned()],
        "{:?}",
        pass.report
    );
    assert!(!data.join("sessions").join("old").exists());
    assert!(!data.join("journal").join("old").exists());
    assert!(
        !data.join("old").exists(),
        "store session dir removed by gc"
    );
    // Live-host and pinned sessions never lose their dirs.
    assert!(data.join("sessions").join("live").is_dir());
    assert!(data.join("journal").join("live").is_dir());
    assert!(data.join("sessions").join("pinned").is_dir());
    assert!(data.join("journal").join("pinned").is_dir());
    // Store truth: gc'd gone, pinned intact.
    let store = SessionStore::open(data).unwrap();
    assert!(matches!(
        store.open_session("old"),
        Err(SessionError::NotFound { .. })
    ));
    assert!(store.open_session("pinned").is_ok());
    assert!(!pass.vacuumed, "vacuum is opt-in");
}

#[tokio::test]
async fn janitor_default_policy_touches_nothing() {
    let fixture = state_with(|c| c);
    let data = &fixture.state.config.data_dir;
    seed_session(data, "kept");
    fake_dirs(data, "kept");

    let pass = janitor::run_once(&fixture.state).unwrap();

    assert!(pass.report.archived.is_empty(), "{:?}", pass.report);
    assert!(pass.report.gc_removed.is_empty());
    assert!(!pass.vacuumed);
    assert!(data.join("sessions").join("kept").is_dir());
    assert!(data.join("journal").join("kept").is_dir());
    let metas: Vec<SessionMeta> = SessionStore::open(data).unwrap().list().unwrap();
    assert_eq!(metas.len(), 1, "the store row must survive");
    assert_eq!(metas[0].id, "kept");
}

#[tokio::test]
async fn janitor_vacuums_only_after_a_deleting_pass_when_configured() {
    let fixture = state_with(|c| DaemonConfig {
        retention: aggressive(),
        retention_vacuum: true,
        ..c
    });
    let state = &fixture.state;
    let data = &state.config.data_dir;
    seed_session(data, "old");
    seed_session(data, "pinned");
    {
        let store = SessionStore::open(data).unwrap();
        store.set_pinned("pinned", true).unwrap();
    }
    fake_dirs(data, "old");
    fake_dirs(data, "pinned");

    let pass = janitor::run_once(state).unwrap();
    assert!(pass.vacuumed, "archiving deleted rows: vacuum must run");
    let pass = janitor::run_once(state).unwrap();
    assert!(pass.vacuumed, "gc deleted rows: vacuum must run");
    let pass = janitor::run_once(state).unwrap();
    assert!(
        !pass.vacuumed,
        "nothing deleted on an empty pass: no vacuum"
    );
    // The store survives the vacuum intact.
    let store = SessionStore::open(data).unwrap();
    assert!(store.open_session("pinned").is_ok());
    assert!(matches!(
        store.open_session("old"),
        Err(SessionError::NotFound { .. })
    ));
}

#[tokio::test]
async fn janitor_cleanup_is_best_effort_and_never_panics() {
    let fixture = state_with(|c| DaemonConfig {
        retention: aggressive(),
        ..c
    });
    let data = &fixture.state.config.data_dir;
    seed_session(data, "dirred");
    seed_session(data, "filed");
    fake_dirs(data, "dirred");
    // A FILE where the engine session dir would be: dir removal
    // fails (not a directory) — the pass must log and continue.
    std::fs::write(data.join("sessions").join("filed"), "not a dir").unwrap();
    std::fs::create_dir_all(data.join("journal").join("filed")).unwrap();

    janitor::run_once(&fixture.state).unwrap();
    janitor::run_once(&fixture.state).unwrap();

    assert!(!data.join("sessions").join("dirred").exists());
    assert!(!data.join("journal").join("dirred").exists());
    assert!(
        data.join("sessions").join("filed").exists(),
        "a failed removal is logged, never fatal"
    );
    assert!(
        !data.join("journal").join("filed").exists(),
        "cleanup continues past per-directory failures"
    );
}

#[test]
fn janitor_shutdown_removes_only_this_instances_readiness_files() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();

    // Our own files: removed.
    std::fs::write(data.join("daemon-url"), "http://127.0.0.1:4242").unwrap();
    std::fs::write(data.join("daemon-token"), "tok-a").unwrap();
    janitor::remove_instance_files(data, "http://127.0.0.1:4242", "tok-a");
    assert!(!data.join("daemon-url").exists());
    assert!(!data.join("daemon-token").exists());

    // A successor's files: not ours to remove.
    std::fs::write(data.join("daemon-url"), "http://127.0.0.1:9999").unwrap();
    std::fs::write(data.join("daemon-token"), "tok-b").unwrap();
    janitor::remove_instance_files(data, "http://127.0.0.1:4242", "tok-a");
    assert!(data.join("daemon-url").exists());
    assert!(data.join("daemon-token").exists());

    // Mixed: our url, a successor's token — only ours goes.
    std::fs::write(data.join("daemon-url"), "http://127.0.0.1:4242").unwrap();
    janitor::remove_instance_files(data, "http://127.0.0.1:4242", "tok-a");
    assert!(!data.join("daemon-url").exists());
    assert!(data.join("daemon-token").exists());

    // Absent files: a no-op, never an error.
    janitor::remove_instance_files(data, "http://127.0.0.1:4242", "tok-a");
}
