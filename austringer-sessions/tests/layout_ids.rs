//! Layout contract, id validation, duplicate/not-found typed errors.

use austringer_core::event::{ContentBlock, Event};
use austringer_sessions::{ImportOrigin, LeaseOwner, SessionError, SessionStore};

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

#[test]
fn session_directory_contract_materializes_layout() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("contract", "t").unwrap();
    let dir = root.path().join("contract");
    assert!(dir.join("checkpoints").is_dir());
    assert!(dir.join("files").is_dir());
    assert!(dir.join("plan.md").is_file());
    assert!(dir.join("events.jsonl").is_file());
    // Import materializes the same contract.
    store
        .import_events("imported-1", "t", &[user("hi")], &ImportOrigin::jsonl())
        .unwrap();
    assert!(root.path().join("imported-1").join("checkpoints").is_dir());
    assert!(root.path().join("imported-1").join("plan.md").is_file());
}

#[test]
fn session_ids_reject_path_breakers_and_accept_unicode() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    for bad in [
        "", "a/b", "a\\b", "..", ".", "a:b", "store.db", "archive", "CON", " lead", "dot.",
    ] {
        assert!(store.create(bad, "").is_err(), "{bad:?} must be rejected");
    }
    store.create("会话-42-λ", "unicode id").unwrap();
    assert!(store.open_session("会话-42-λ").is_ok());
}

#[test]
fn duplicate_create_is_typed_already_exists() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("dup", "").unwrap();
    assert!(matches!(
        store.create("dup", ""),
        Err(SessionError::AlreadyExists { .. })
    ));
}

#[test]
fn open_unknown_session_is_typed_not_found() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    assert!(matches!(
        store.open_session("ghost"),
        Err(SessionError::NotFound { .. })
    ));
}

#[test]
fn list_reports_sessions_newest_first_with_counters() {
    let root = tempfile::tempdir().unwrap();
    let (clock, tick) = {
        let tick = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1_000));
        let clone = tick.clone();
        let clock: austringer_sessions::Clock =
            std::sync::Arc::new(move || clone.load(std::sync::atomic::Ordering::SeqCst));
        (clock, tick)
    };
    let store = SessionStore::open_with_clock(root.path(), clock).unwrap();
    store.create("older", "").unwrap();
    tick.store(2_000, std::sync::atomic::Ordering::SeqCst);
    store.create("newer", "").unwrap();
    let h = store
        .open_session_rw("newer", &LeaseOwner::new("w"))
        .unwrap();
    h.append(user("content")).unwrap();
    let listed = store.list().unwrap();
    assert_eq!(listed[0].id, "newer");
    assert_eq!(listed[0].event_count, 1);
    assert_eq!(listed[1].id, "older");
    assert_eq!(listed[1].event_count, 0);
}
