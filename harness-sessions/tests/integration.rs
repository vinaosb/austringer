//! Store lifecycle: create/append, lazy open, resume, fork, export.

use harness_core::event::{ContentBlock, Event, RecordedEvent, Sequence};
use harness_sessions::{
    Clock, EventSearchHit, ForkOptions, ImportOrigin, LeaseOwner, OpenMode, SessionError,
    SessionStore,
};
use std::time::Instant;

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

fn assistant(text: &str) -> Event {
    Event::Assistant {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

fn test_clock(start: u64) -> (Clock, std::sync::Arc<std::sync::atomic::AtomicU64>) {
    let tick = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(start));
    let clone = tick.clone();
    (
        std::sync::Arc::new(move || clone.load(std::sync::atomic::Ordering::SeqCst)) as Clock,
        tick,
    )
}

#[test]
fn create_append_and_lazy_open_round_trip() {
    let root = tempfile::tempdir().unwrap();
    let (clock, _tick) = test_clock(1_000);
    let store = SessionStore::open_with_clock(root.path(), clock).unwrap();

    store.create("alpha", "first session").unwrap();
    {
        let h = store
            .open_session_rw("alpha", &LeaseOwner::new("writer-1"))
            .unwrap();
        assert_eq!(h.last_sequence(), Sequence(0));
        h.append(user("hello harness")).unwrap();
        h.append(assistant("hello user")).unwrap();
        h.append(Event::ToolCall {
            call_id: "c1".to_owned(),
            tool: "edit_lines".to_owned(),
            arguments: serde_json::json!({"path": "a.rs"}),
        })
        .unwrap();
        h.append(Event::ToolResult {
            call_id: "c1".to_owned(),
            blocks: vec![ContentBlock::Text {
                text: "ok".to_owned(),
            }],
        })
        .unwrap();
    }

    // Lazy open: metadata + index bounds only; nothing deserialized yet.
    let ro = store.open_session("alpha").unwrap();
    let meta = ro.meta();
    assert_eq!(meta.event_count, 4);
    assert_eq!(meta.turn_count, 1);
    assert_eq!(ro.last_sequence(), Sequence(4));

    // Two turns after a second user event.
    {
        let h = store
            .open_session_rw("alpha", &LeaseOwner::new("writer-1"))
            .unwrap();
        h.append(user("second turn question")).unwrap();
        h.append(assistant("second turn answer")).unwrap();
    }

    let ro = store.open_session("alpha").unwrap();
    assert_eq!(ro.meta().turn_count, 2);

    // Turn paging returns CONTENT, not counts.
    let turn1 = ro.turn(1).unwrap();
    assert_eq!(turn1.events.len(), 4);
    assert_eq!(
        turn1.events[0].event,
        user("hello harness"),
        "page-in must return the actual events"
    );
    let turn2 = ro.turn(2).unwrap();
    assert_eq!(turn2.events.len(), 2);
    assert_eq!(turn2.events[0].event, user("second turn question"));

    let page = ro.turns_range(1, 2).unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[1].turn, 2);

    // Sequence-range paging is available independently of turns.
    let events = ro.events_range(Sequence(2), Sequence(3)).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event, assistant("hello user"));
}

#[test]
fn resume_appends_with_continued_sequence_after_reopen() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("resumable", "").unwrap();
    {
        let h = store
            .open_session_rw("resumable", &LeaseOwner::new("w"))
            .unwrap();
        for i in 0..5 {
            h.append(user(&format!("turn {i}"))).unwrap();
        }
    }
    // Simulate a restart: a brand-new store handle over the same root.
    let store2 = SessionStore::open(root.path()).unwrap();
    {
        let h = store2
            .open_session_rw("resumable", &LeaseOwner::new("w2"))
            .unwrap();
        let seq = h.append(assistant("resumed")).unwrap();
        assert_eq!(seq, Sequence(6), "resume must continue the sequence");
    }
    let ro = store2.open_session("resumable").unwrap();
    assert_eq!(ro.meta().event_count, 6);
    let last = ro.events_range(Sequence(6), Sequence(6)).unwrap();
    assert_eq!(last[0].event, assistant("resumed"));
}

#[test]
fn fork_copies_prefix_and_diverges() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("base", "").unwrap();
    {
        let h = store
            .open_session_rw("base", &LeaseOwner::new("w"))
            .unwrap();
        h.append(user("shared question")).unwrap();
        h.append(assistant("shared answer")).unwrap();
        h.append(user("original continuation")).unwrap();
        h.append(assistant("original tail")).unwrap();
    }

    let fork_meta = store
        .fork("fork-1", "base", Sequence(2), &ForkOptions::default())
        .unwrap();
    assert_eq!(fork_meta.event_count, 2);
    assert_eq!(fork_meta.turn_count, 1);
    assert_eq!(fork_meta.forked_from.as_deref(), Some("base"));

    // Diverge the fork.
    {
        let h = store
            .open_session_rw("fork-1", &LeaseOwner::new("w"))
            .unwrap();
        let seq = h.append(user("forked continuation")).unwrap();
        assert_eq!(seq, Sequence(3));
    }
    // Base is untouched.
    let base = store.open_session("base").unwrap();
    assert_eq!(base.meta().event_count, 4);
    let fork = store.open_session("fork-1").unwrap();
    assert_eq!(fork.meta().event_count, 3);
    let tail = fork.events_range(Sequence(3), Sequence(3)).unwrap();
    assert_eq!(tail[0].event, user("forked continuation"));

    // Fork point beyond the log is a typed error.
    let err = store.fork("fork-2", "base", Sequence(99), &ForkOptions::default());
    assert!(matches!(err, Err(SessionError::ForkPointBeyondLog { .. })));
}

#[test]
fn export_jsonl_round_trips_the_t4_line_format() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("export-me", "").unwrap();
    {
        let h = store
            .open_session_rw("export-me", &LeaseOwner::new("w"))
            .unwrap();
        h.append(user("exported question")).unwrap();
        h.append(assistant("exported answer")).unwrap();
    }
    let ro = store.open_session("export-me").unwrap();
    let exported = ro.export_jsonl().unwrap();
    let content = std::fs::read_to_string(&exported).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    let first: RecordedEvent = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first.event, user("exported question"));
    assert_eq!(first.seq, Sequence(1));

    // Re-import the exported file into a fresh session and compare events.
    store
        .import_jsonl("reimported", "", exported.as_path(), &ImportOrigin::jsonl())
        .unwrap();
    let back = store.open_session("reimported").unwrap();
    let events = back.events_range(Sequence(1), Sequence(2)).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event, user("exported question"));
}

#[test]
fn cross_session_fts_search_finds_events_in_other_sessions() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    for (id, q, a) in [
        ("s1", "how do i bake bread", "use flour"),
        ("s2", "how do i weld steel", "use a torch"),
        ("s3", "rust borrow checker", "lifetimes"),
    ] {
        store.create(id, "").unwrap();
        let h = store.open_session_rw(id, &LeaseOwner::new("w")).unwrap();
        h.append(user(q)).unwrap();
        h.append(assistant(a)).unwrap();
    }
    let hits: Vec<EventSearchHit> = store.search_events("bread", 10).unwrap();
    assert!(
        hits.iter().any(|h| h.session_id == "s1"),
        "expected s1 in {hits:?}"
    );
    assert!(hits.iter().all(|h| h.session_id != "s3"));
    // Snippets carry CONTENT.
    assert!(hits[0].snippet.contains("bread"));
}

#[test]
fn open_is_metadata_only_on_a_large_session() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("big", "").unwrap();
    let big_text = "x".repeat(64 * 1024);
    {
        let h = store.open_session_rw("big", &LeaseOwner::new("w")).unwrap();
        for i in 0..4 {
            h.append(user(&format!("{i} {big_text}"))).unwrap();
        }
    }
    let ro = store.open_session("big").unwrap();
    let start = Instant::now();
    let _meta = store.open_session("big").unwrap();
    let _ = ro.meta();
    let elapsed = start.elapsed();
    // Deserializing even 256 KiB of payload would take far longer than
    // this on any build; the budget is generous to stay CI-stable while
    // still failing if open ever scans payloads.
    assert!(
        elapsed.as_millis() < 100,
        "open took {elapsed:?}; open() must not read payloads"
    );
}

#[test]
fn read_only_handle_rejects_appends_typed() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("ro", "").unwrap();
    let ro = store.open_session("ro").unwrap();
    let err = ro.append(user("nope")).unwrap_err();
    assert!(matches!(err, SessionError::NotWritable { .. }));
}

#[test]
fn open_mode_read_write_is_the_documented_default_for_writers() {
    // OpenMode is part of the public API surface: ReadWrite acquires a
    // lease, ReadOnly never touches the lease table.
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("modes", "").unwrap();
    let _ = store
        .open_session_mode("modes", OpenMode::ReadOnly)
        .unwrap();
    let _ = store
        .open_session_mode("modes", OpenMode::ReadWrite(LeaseOwner::new("w")))
        .unwrap();
}
