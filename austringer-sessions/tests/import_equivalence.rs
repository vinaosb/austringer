//! Import equivalence: a JSONL import lands the same rows the append
//! path writes — events, turn index, and the FTS index.

use austringer_core::event::{ContentBlock, Event};
use austringer_sessions::{ImportOrigin, LeaseOwner, SessionStore};

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

#[test]
fn import_jsonl_preserves_rows_and_fts_presence() {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    store.create("source", "").unwrap();
    {
        let h = store
            .open_session_rw("source", &LeaseOwner::new("w"))
            .unwrap();
        h.append(user("needle question")).unwrap();
        h.append(assistant("plain answer")).unwrap();
        h.append(user("second turn needle")).unwrap();
    }
    let ro = store.open_session("source").unwrap();
    let exported = ro.export_jsonl().unwrap();
    let report = store
        .import_jsonl("imported", "", exported.as_path(), &ImportOrigin::jsonl())
        .unwrap();

    assert_eq!(report.meta.event_count, 3);
    assert_eq!(report.meta.turn_count, 2);
    assert_eq!(report.meta.last_seq, 3);
    assert!(report.meta.payload_bytes > 0);

    // Row-for-row storage equivalence through a second connection.
    {
        let conn = rusqlite::Connection::open(root.path().join("store.db")).unwrap();
        let events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE session_id = 'imported'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 3);
        let fts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM event_fts WHERE session_id = 'imported'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts, 3, "every imported text event must carry an FTS row");
        let turns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM turn_index WHERE session_id = 'imported'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(turns, 2);
    }

    // The imported session's text is live in cross-session search.
    let hits = store.search_events("needle", 10).unwrap();
    assert!(
        hits.iter().any(|h| h.session_id == "imported"),
        "imported session must appear in FTS search: {hits:?}"
    );
}
