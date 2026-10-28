//! Retention reclamation: prune's deletions must reach the disk. The
//! WAL is checkpointed and truncated after a deleting run, VACUUM is
//! the caller's opt-in for page reclaim, and the prune report exposes
//! the session ids a future janitor needs to clean out-of-store state.

use std::path::Path;
use std::sync::Arc;

use austringer_core::event::{ContentBlock, Event};
use austringer_sessions::{
    Clock, FakeLeaseProbe, LeaseOwner, RetentionPolicy, SessionError, SessionStore,
};

fn clock_at(start: u64) -> (Clock, Arc<std::sync::atomic::AtomicU64>) {
    let tick = Arc::new(std::sync::atomic::AtomicU64::new(start));
    let clone = tick.clone();
    (
        Arc::new(move || clone.load(std::sync::atomic::Ordering::SeqCst)) as Clock,
        tick,
    )
}

fn advance(tick: &Arc<std::sync::atomic::AtomicU64>, to: u64) {
    tick.store(to, std::sync::atomic::Ordering::SeqCst);
}

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

/// Appends enough payload (3 sessions x 24 x 96 KB, events + FTS text)
/// to push the WAL past SQLite's 1000-page auto-checkpoint, so its
/// high-water file size is nontrivial.
fn grow_store(store: &SessionStore, owner: &LeaseOwner) {
    let big = "w".repeat(96 * 1024);
    for id in ["bulk-a", "bulk-b", "bulk-c"] {
        store.create(id, "").unwrap();
        let h = store.open_session_rw(id, owner).unwrap();
        for _ in 0..24 {
            h.append(user(&big)).unwrap();
        }
    }
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

#[test]
fn prune_truncates_wal_after_deleting() {
    let root = tempfile::tempdir().unwrap();
    let (clock, _tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    let owner = LeaseOwner::new("w");
    grow_store(&store, &owner);
    let wal = root.path().join("store.db-wal");
    let wal_before = file_len(&wal);
    assert!(
        wal_before > 1024 * 1024,
        "premise: WAL must be nontrivial before prune, got {wal_before} bytes"
    );
    let report = store
        .prune(&RetentionPolicy {
            archive_after_secs: Some(0),
            max_store_bytes: None,
            gc_archived_after_secs: None,
        })
        .unwrap();
    assert_eq!(
        report.archived.len(),
        3,
        "all bulk sessions archive: {report:?}"
    );
    assert_eq!(
        file_len(&wal),
        0,
        "prune must wal_checkpoint(TRUNCATE) after deleting (WAL was {wal_before} bytes)"
    );
    // Session data is gone per the existing archive semantics.
    assert!(matches!(
        store.open_session_rw("bulk-a", &owner),
        Err(SessionError::Archived { .. })
    ));
}

#[test]
fn vacuum_is_opt_in_and_reclaims_freed_pages() {
    let root = tempfile::tempdir().unwrap();
    let (clock, _tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    let owner = LeaseOwner::new("w");
    grow_store(&store, &owner);
    store
        .prune(&RetentionPolicy {
            archive_after_secs: Some(0),
            max_store_bytes: None,
            gc_archived_after_secs: None,
        })
        .unwrap();
    let db = root.path().join("store.db");
    let db_before = file_len(&db);
    assert!(
        db_before > 1024 * 1024,
        "premise: prune must not vacuum on its own (opt-in contract); \
         store.db still holds the freed pages, got {db_before} bytes"
    );
    store.vacuum().unwrap();
    let db_after = file_len(&db);
    assert!(
        db_after * 4 < db_before,
        "vacuum must return the freed pages to the filesystem: {db_before} -> {db_after}"
    );
    assert_eq!(
        file_len(&root.path().join("store.db-wal")),
        0,
        "vacuum must checkpoint the rebuilt image out of the WAL"
    );
    // The store stays fully functional after the rebuild.
    store.create("post-vacuum", "").unwrap();
    let h = store.open_session_rw("post-vacuum", &owner).unwrap();
    h.append(user("alive")).unwrap();
    assert!(matches!(
        store.open_session_rw("bulk-a", &owner),
        Err(SessionError::Archived { .. })
    ));
}

#[test]
fn report_exposes_archived_and_gc_ids() {
    let root = tempfile::tempdir().unwrap();
    let (clock, tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    let owner = LeaseOwner::new("w");
    store.create("s-one", "").unwrap();
    let h = store.open_session_rw("s-one", &owner).unwrap();
    h.append(user("one")).unwrap();
    drop(h);
    advance(&tick, 1_000_050);
    store.create("s-two", "").unwrap();
    let h = store.open_session_rw("s-two", &owner).unwrap();
    h.append(user("two")).unwrap();
    drop(h);
    advance(&tick, 1_000_100);
    let report = store
        .prune(&RetentionPolicy {
            // Cutoff 1_000_049: only s-one (updated_at 1_000_000) is old
            // enough; s-two (updated_at 1_000_050) stays.
            archive_after_secs: Some(51),
            max_store_bytes: None,
            gc_archived_after_secs: None,
        })
        .unwrap();
    let archived_ids: Vec<&str> = report.archived.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        archived_ids,
        vec!["s-one"],
        "janitor needs the archived ids"
    );
    assert_eq!(report.archived[0].events, 1);
    // GC tier: the follow-up prune must expose which cold files it removed.
    advance(&tick, 1_000_200);
    let report = store
        .prune(&RetentionPolicy {
            archive_after_secs: None,
            max_store_bytes: None,
            // Cutoff 1_000_100 == s-one's archived_at (inclusive boundary).
            gc_archived_after_secs: Some(100),
        })
        .unwrap();
    assert_eq!(
        report.gc_removed,
        vec!["s-one".to_owned()],
        "janitor needs the gc'd ids"
    );
    assert!(matches!(
        store.open_session("s-one"),
        Err(SessionError::NotFound { .. })
    ));
}
