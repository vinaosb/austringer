//! `/rewind` log truncation: the session store's reset-to-head contract
//! (plan todo 5). One transaction must drop events, FTS rows, and turn
//! index rows above the head and reset `last_seq`/`event_count`/
//! `turn_count`/`payload_bytes` so the next append continues at head+1
//! and a fork at the head copies exactly the surviving prefix â€” search
//! can never return un-happened events.

use harness_core::event::{ContentBlock, Event, Sequence};
use harness_sessions::lease::LeaseOwner;
use harness_sessions::{ForkOptions, SessionError, SessionStore};

fn user(text: &str) -> Event {
    Event::User {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
    }
}

/// Three turns: user text + snapshot marker each. Every user text
/// carries the shared FTS term `needle`; turn 3's text is unique.
fn fixture_events() -> Vec<Event> {
    vec![
        user("needle turn one"),
        Event::Snapshot {
            label: "turn-1".into(),
        },
        user("needle turn two"),
        Event::Snapshot {
            label: "turn-2".into(),
        },
        user("needle turn three unique"),
        Event::Snapshot {
            label: "turn-3".into(),
        },
    ]
}

fn populated(root: &std::path::Path) -> SessionStore {
    let store = SessionStore::open(root).unwrap();
    store.create("s", "truncate fixture").unwrap();
    {
        let handle = store
            .open_session_rw("s", &LeaseOwner::new("fixture"))
            .unwrap();
        handle.append_batch(&fixture_events()).unwrap();
    }
    store
}

#[test]
fn turn_end_seq_maps_turn_boundaries() {
    let root = tempfile::tempdir().unwrap();
    let store = populated(root.path());
    assert_eq!(store.turn_end_seq("s", 1).unwrap(), Some(2));
    assert_eq!(store.turn_end_seq("s", 2).unwrap(), Some(4));
    assert_eq!(store.turn_end_seq("s", 3).unwrap(), Some(6));
    assert_eq!(store.turn_end_seq("s", 9).unwrap(), None);
}

#[test]
fn truncate_drops_fts_above_head_and_resets_counters() {
    let root = tempfile::tempdir().unwrap();
    let store = populated(root.path());

    {
        let handle = store
            .open_session_rw("s", &LeaseOwner::new("rewind"))
            .unwrap();
        handle.truncate_to(Sequence(3)).unwrap();
    }

    let meta = store.open_session("s").unwrap().meta().clone();
    assert_eq!(meta.last_seq, 3, "head is the restored boundary");
    assert_eq!(meta.event_count, 3);
    assert_eq!(meta.turn_count, 2, "turn 3 (start seq 5) is gone");
    let payload_bytes: u64 = {
        let ro = store.open_session("s").unwrap();
        let events = ro.events_range(Sequence(1), Sequence(1_000_000_000)).unwrap();
        assert_eq!(events.len(), 3, "no events survive above the head");
        assert_eq!(events.last().unwrap().seq, Sequence(3));
        events
            .iter()
            .map(|recorded| serde_json::to_vec(&recorded).unwrap().len() as u64)
            .sum()
    };
    assert_eq!(meta.payload_bytes, payload_bytes);

    // FTS holds no rows above the head: the shared term only hits
    // surviving seqs, and turn 3's unique term is gone entirely.
    let hits = store.search_events("needle", 50).unwrap();
    let seqs: Vec<u64> = hits.iter().map(|hit| hit.seq).collect();
    assert_eq!(seqs, vec![1, 3], "search never returns un-happened events");
    assert!(
        store.search_events("unique", 50).unwrap().is_empty(),
        "turn 3 text must not be searchable after the rewind"
    );
}

#[test]
fn next_append_continues_at_head_plus_one_without_gaps() {
    let root = tempfile::tempdir().unwrap();
    let store = populated(root.path());

    {
        let handle = store
            .open_session_rw("s", &LeaseOwner::new("rewind"))
            .unwrap();
        handle.truncate_to(Sequence(3)).unwrap();
    }

    {
        let handle = store
            .open_session_rw("s", &LeaseOwner::new("after"))
            .unwrap();
        assert_eq!(handle.last_sequence(), Sequence(3));
        let first = handle.append(user("needle after rewind")).unwrap();
        assert_eq!(first, Sequence(4), "next append is head+1");
        let second = handle.append(Event::System { text: "again".into() }).unwrap();
        assert_eq!(second, Sequence(5), "the sequence never skips or repeats");
    }

    let ro = store.open_session("s").unwrap();
    let events = ro.events_range(Sequence(1), Sequence(1_000_000_000)).unwrap();
    let seqs: Vec<u64> = events.iter().map(|recorded| recorded.seq.0).collect();
    assert_eq!(seqs, vec![1, 2, 3, 4, 5]);
    // verify() ran at open: index agrees with the truncated + extended log.
    assert_eq!(ro.meta().turn_count, 3, "post-rewind appends open turn 3");
}

#[test]
fn fork_at_post_rewind_head_copies_only_surviving_events() {
    let root = tempfile::tempdir().unwrap();
    let store = populated(root.path());

    {
        let handle = store
            .open_session_rw("s", &LeaseOwner::new("rewind"))
            .unwrap();
        handle.truncate_to(Sequence(3)).unwrap();
    }

    store
        .fork("f", "s", Sequence(3), &ForkOptions::default())
        .unwrap();
    let fork_meta = store.open_session("f").unwrap().meta().clone();
    assert_eq!(fork_meta.last_seq, 3);
    assert_eq!(fork_meta.event_count, 3);

    let (source, forked) = (store.open_session("s").unwrap(), store.open_session("f").unwrap());
    let source_events = source.events_range(Sequence(1), Sequence(1_000_000_000)).unwrap();
    let fork_events = forked.events_range(Sequence(1), Sequence(1_000_000_000)).unwrap();
    assert_eq!(source_events, fork_events, "fork copies only events <= head");
}

#[test]
fn truncate_beyond_log_is_a_typed_refusal() {
    let root = tempfile::tempdir().unwrap();
    let store = populated(root.path());
    let handle = store
        .open_session_rw("s", &LeaseOwner::new("rewind"))
        .unwrap();
    let refusal = handle.truncate_to(Sequence(7)).unwrap_err();
    match refusal {
        SessionError::TruncateBeyondLog { at, last, .. } => {
            assert_eq!((at, last), (7, 6));
        }
        other => panic!("expected typed TruncateBeyondLog, got {other:?}"),
    }
    // The refusal changed nothing.
    assert_eq!(handle.last_sequence(), Sequence(6));
}

#[test]
fn truncate_to_zero_empties_the_log_and_index() {
    let root = tempfile::tempdir().unwrap();
    let store = populated(root.path());

    {
        let handle = store
            .open_session_rw("s", &LeaseOwner::new("rewind"))
            .unwrap();
        handle.truncate_to(Sequence(0)).unwrap();
    }

    let meta = store.open_session("s").unwrap().meta().clone();
    assert_eq!((meta.last_seq, meta.event_count, meta.turn_count), (0, 0, 0));
    assert!(store.search_events("needle", 50).unwrap().is_empty());
    let ro = store.open_session("s").unwrap();
    assert!(ro.events_range(Sequence(1), Sequence(1_000_000_000)).unwrap().is_empty());
}

#[test]
fn read_only_handle_cannot_truncate() {
    let root = tempfile::tempdir().unwrap();
    let store = populated(root.path());
    let ro = store.open_session("s").unwrap();
    match ro.truncate_to(Sequence(3)) {
        Err(SessionError::NotWritable { .. }) => {}
        other => panic!("expected typed NotWritable, got {other:?}"),
    }
}
