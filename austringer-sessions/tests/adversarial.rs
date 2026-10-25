//! Adversarial inputs: corrupt database, corrupt index, damaged event
//! log, torn imports, huge single turns.

use std::sync::Arc;

use austringer_core::event::{ContentBlock, Event, Sequence};
use austringer_sessions::{Clock, FakeLeaseProbe, LeaseOwner, SessionError, SessionStore};

fn clock_at(start: u64) -> Clock {
    let tick = Arc::new(std::sync::atomic::AtomicU64::new(start));
    Arc::new(move || tick.load(std::sync::atomic::Ordering::SeqCst))
}

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

#[test]
fn corrupt_database_file_is_a_typed_error() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("store.db"),
        b"this is definitely not a sqlite database",
    )
    .unwrap();
    let error = SessionStore::open(root.path()).unwrap_err();
    assert!(
        matches!(error, SessionError::Corrupt { .. }),
        "expected Corrupt, got {error:?}"
    );
}

#[test]
fn corrupt_turn_index_is_detected_and_repairable() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000), probe).unwrap();
    store.create("corruptible", "").unwrap();
    let owner = LeaseOwner::new("w");
    {
        let h = store.open_session_rw("corruptible", &owner).unwrap();
        h.append(user("q1")).unwrap();
        h.append(Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: "a1".to_owned(),
            }],
        })
        .unwrap();
        h.append(user("q2")).unwrap();
    }
    // Interior corruption: delete one index row through a second
    // connection (the crate never exposes its own).
    {
        let conn = rusqlite::Connection::open(root.path().join("store.db")).unwrap();
        conn.execute(
            "DELETE FROM turn_index WHERE session_id = 'corruptible' AND turn = 1",
            [],
        )
        .unwrap();
    }
    let ro = store.open_session("corruptible").unwrap();
    let error = ro.turns_range(1, 2).unwrap_err();
    assert!(
        matches!(error, SessionError::CorruptIndex { .. }),
        "interior index loss must be typed: {error:?}"
    );
    let turns = ro.repair_index().unwrap();
    assert_eq!(turns, 2);
    let repaired = ro.turns_range(1, 2).unwrap();
    assert_eq!(repaired.len(), 2);
    assert_eq!(repaired[0].events.len(), 2);
    assert_eq!(repaired[0].events[0].event, user("q1"));
}

#[test]
fn damaged_event_log_refuses_index_repair_typed() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000), probe).unwrap();
    store.create("gapped", "").unwrap();
    let owner = LeaseOwner::new("w");
    {
        let h = store.open_session_rw("gapped", &owner).unwrap();
        h.append(user("one")).unwrap();
        h.append(user("two")).unwrap();
    }
    {
        let conn = rusqlite::Connection::open(root.path().join("store.db")).unwrap();
        conn.execute(
            "DELETE FROM events WHERE session_id = 'gapped' AND seq = 1",
            [],
        )
        .unwrap();
    }
    let ro = store.open_session("gapped").unwrap();
    let error = ro.repair_index().unwrap_err();
    assert!(
        matches!(error, SessionError::EventLogDamaged { .. }),
        "sequence gaps must refuse rebuild: {error:?}"
    );
}

#[test]
fn torn_jsonl_import_reports_and_continues() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    let file = root.path().join("torn.jsonl");
    let body = concat!(
        r#"{"seq":1,"event":{"type":"user","blocks":[{"type":"text","text":"kept"}]}}"#,
        "\n",
        "this line is not json\n",
        r#"{"seq":2,"event":{"type":"user","blocks":[{"type":"text","text":"torn"#,
    );
    std::fs::write(&file, body).unwrap();
    let report = store
        .import_jsonl(
            "torn-import",
            "",
            &file,
            &austringer_sessions::ImportOrigin::jsonl(),
        )
        .unwrap();
    assert_eq!(report.meta.event_count, 1);
    assert_eq!(report.invalid_lines, 1);
    assert!(report.torn_tail);
    let ro = store.open_session("torn-import").unwrap();
    let events = ro.events_range(Sequence(1), Sequence(1)).unwrap();
    assert_eq!(events[0].event, user("kept"));
}

#[test]
fn huge_single_turn_pages_in_with_content() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("huge-turn", "").unwrap();
    let big = "x".repeat(2 * 1024 * 1024);
    let owner = LeaseOwner::new("w");
    {
        let h = store.open_session_rw("huge-turn", &owner).unwrap();
        h.append(user(&big)).unwrap();
        h.append(Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: "done".to_owned(),
            }],
        })
        .unwrap();
    }
    let ro = store.open_session("huge-turn").unwrap();
    let turn = ro.turn(1).unwrap();
    assert_eq!(turn.events.len(), 2);
    assert_eq!(turn.events[0].event, user(&big));
    assert!(ro.meta().payload_bytes > 2 * 1024 * 1024);
}
