//! Retention: age/size tiers, pinned exemption, GC, explicit-only
//! deletion — with injected clocks at the boundaries.

use std::sync::Arc;

use harness_core::event::{ContentBlock, Event};
use harness_sessions::{
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

#[test]
fn age_boundary_is_inclusive_and_pinned_exempt() {
    let root = tempfile::tempdir().unwrap();
    let (clock, tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    store.create("old", "").unwrap();
    store.create("pinned-old", "").unwrap();
    store.set_pinned("pinned-old", true).unwrap();
    let owner = LeaseOwner::new("w");
    for id in ["old", "pinned-old"] {
        let h = store.open_session_rw(id, &owner).unwrap();
        h.append(user(&format!("{id} content"))).unwrap();
    }
    // Exactly at the boundary: age == archive_after.
    advance(&tick, 1_000_000 + 86_400);
    let report = store
        .prune(&RetentionPolicy {
            archive_after_secs: Some(86_400),
            max_store_bytes: None,
            gc_archived_after_secs: None,
        })
        .unwrap();
    assert_eq!(
        report.archived.len(),
        1,
        "exactly-at-boundary archives: {report:?}"
    );
    assert_eq!(report.archived[0].id, "old");
    assert_eq!(report.pinned_skipped, vec!["pinned-old".to_owned()]);
    // The archive file holds the CONTENT.
    let cold = std::fs::read_to_string(&report.archived[0].path).unwrap();
    assert!(cold.contains("old content"), "cold file must carry events");
    // Archived sessions refuse appends typed.
    assert!(matches!(
        store.open_session_rw("old", &owner),
        Err(SessionError::Archived { .. })
    ));
    // One second before the boundary: nothing archives.
    store.create("young", "").unwrap();
    let h = store.open_session_rw("young", &owner).unwrap();
    h.append(user("fresh")).unwrap();
    advance(&tick, 1_000_000 + 86_400 + 100);
    let report = store
        .prune(&RetentionPolicy {
            archive_after_secs: Some(101),
            max_store_bytes: None,
            gc_archived_after_secs: None,
        })
        .unwrap();
    assert!(report.archived.is_empty(), "young session must not archive");
}

#[test]
fn gc_removes_cold_files_after_boundary_and_empty_policy_deletes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let (clock, tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    store.create("cold", "").unwrap();
    let owner = LeaseOwner::new("w");
    {
        let h = store.open_session_rw("cold", &owner).unwrap();
        h.append(user("cold content")).unwrap();
    }
    advance(&tick, 1_000_000 + 86_400);
    store
        .prune(&RetentionPolicy {
            archive_after_secs: Some(86_400),
            max_store_bytes: None,
            gc_archived_after_secs: Some(86_400),
        })
        .unwrap();
    // Empty policy never deletes.
    let report = store.prune(&RetentionPolicy::default()).unwrap();
    assert!(report.gc_removed.is_empty());
    assert!(report.archived.is_empty());
    // GC tier after its boundary removes file + tombstone.
    advance(&tick, 1_000_000 + 2 * 86_400);
    let report = store
        .prune(&RetentionPolicy {
            archive_after_secs: None,
            max_store_bytes: None,
            gc_archived_after_secs: Some(86_400),
        })
        .unwrap();
    assert_eq!(report.gc_removed, vec!["cold".to_owned()]);
    assert!(matches!(
        store.open_session("cold"),
        Err(SessionError::NotFound { .. })
    ));
}

#[test]
fn size_tier_archives_oldest_unpinned_until_under_budget() {
    let root = tempfile::tempdir().unwrap();
    let (clock, _tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    let owner = LeaseOwner::new("w");
    let big = "z".repeat(40 * 1024);
    for id in ["a", "b", "c"] {
        store.create(id, "").unwrap();
        let h = store.open_session_rw(id, &owner).unwrap();
        h.append(user(&big)).unwrap();
    }
    let report = store
        .prune(&RetentionPolicy {
            archive_after_secs: None,
            max_store_bytes: Some(50 * 1024),
            gc_archived_after_secs: None,
        })
        .unwrap();
    assert_eq!(
        report.archived.len(),
        2,
        "two oldest must archive: {report:?}"
    );
    assert_eq!(report.archived[0].id, "a");
    assert_eq!(report.archived[1].id, "b");
    let remaining: Vec<String> = store
        .list()
        .unwrap()
        .into_iter()
        .filter(|m| m.archived_at.is_none())
        .map(|m| m.id)
        .collect();
    assert_eq!(remaining, vec!["c".to_owned()]);
}
