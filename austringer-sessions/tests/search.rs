//! Todo 11 (ecosystem alignment): the event_fts SELECT surface —
//! cross-session search (existing) plus the new session-scoped
//! variant, ranked by the meta.rs bm25/rank idiom, with typed
//! `BadQuery` for unparseable FTS5 syntax.

use austringer_core::event::{ContentBlock, Event};
use austringer_sessions::{EventSearchHit, LeaseOwner, SessionError, SessionStore};

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

/// Seed one session with the given events under a fresh writer lease.
fn seed(store: &SessionStore, id: &str, events: &[Event]) {
    store.create(id, "").unwrap();
    let h = store.open_session_rw(id, &LeaseOwner::new("seed")).unwrap();
    for event in events {
        h.append(event.clone()).unwrap();
    }
}

fn seeded_store() -> SessionStore {
    let root = tempfile::tempdir().unwrap();
    // Keep the tempdir alive for the store's lifetime (the store opens
    // the SQLite file lazily per call).
    let store = SessionStore::open(root.path()).unwrap();
    std::mem::forget(root);
    store
}

/// A three-session store: s1 mentions the needle once, s2 three times
/// (higher term frequency must rank first), s3 never.
fn three_session_store() -> SessionStore {
    let root = tempfile::tempdir().unwrap();
    let store = SessionStore::open(root.path()).unwrap();
    seed(
        &store,
        "s1",
        &[
            user("unrelated opener"),
            assistant("the needle appears once here"),
        ],
    );
    seed(
        &store,
        "s2",
        &[
            user("needle needle in the haystack"),
            assistant("another needle sighting"),
            assistant("needle once more, third time"),
        ],
    );
    seed(&store, "s3", &[user("no mention at all")]);
    std::mem::forget(root);
    store
}

fn assert_ranked(hits: &[EventSearchHit]) {
    for pair in hits.windows(2) {
        assert!(
            pair[0].bm25 <= pair[1].bm25,
            "hits must arrive bm25-ascending (best first): {hits:?}"
        );
    }
}

#[test]
fn search_ranked_hits_carry_session_ids_and_seq_across_three_sessions() {
    let store = three_session_store();
    let hits = store.search_events("needle", 10).unwrap();
    assert!(!hits.is_empty(), "needle must match s1 and s2");
    assert_ranked(&hits);
    // Term frequency decides: s2 (3 events with "needle") ranks above
    // s1 (1 event), and s3 never appears.
    assert_eq!(
        hits[0].session_id, "s2",
        "tf-rich session ranks first: {hits:?}"
    );
    let sessions: Vec<&str> = hits.iter().map(|h| h.session_id.as_str()).collect();
    assert!(
        sessions.contains(&"s1") && sessions.contains(&"s2"),
        "{hits:?}"
    );
    assert!(!sessions.contains(&"s3"), "{hits:?}");
    // Every hit names a real event position (seq is 1-based).
    for hit in &hits {
        assert!(hit.seq >= 1, "seq must be a 1-based event id: {hit:?}");
        assert!(!hit.snippet.is_empty(), "snippets carry content: {hit:?}");
    }
    // The single s1 hit is seq 2 ("the needle appears once here").
    let s1 = hits.iter().find(|h| h.session_id == "s1").unwrap();
    assert_eq!(s1.seq, 2);
}

#[test]
fn scoped_search_limits_results_to_the_listed_session_ids() {
    let store = three_session_store();
    let hits = store
        .search_events_in_sessions("needle", &["s1"], 10)
        .unwrap();
    assert!(!hits.is_empty(), "s1 contains the needle");
    assert!(
        hits.iter().all(|h| h.session_id == "s1"),
        "scoped search must not leak other sessions: {hits:?}"
    );

    // Two listed ids return hits from both, still ranked.
    let both = store
        .search_events_in_sessions("needle", &["s1", "s2"], 10)
        .unwrap();
    assert_ranked(&both);
    assert!(
        both.iter().any(|h| h.session_id == "s1") && both.iter().any(|h| h.session_id == "s2"),
        "both scopes are searched: {both:?}"
    );

    // A listed id without matches yields an empty (200-shaped) result.
    let none = store
        .search_events_in_sessions("needle", &["s3"], 10)
        .unwrap();
    assert!(none.is_empty(), "s3 has no needle: {none:?}");
}

#[test]
fn scoped_search_with_an_empty_scope_matches_nothing() {
    let store = three_session_store();
    let hits = store.search_events_in_sessions("needle", &[], 10).unwrap();
    assert!(
        hits.is_empty(),
        "no session ids selected means no hits (fail-closed): {hits:?}"
    );
}

#[test]
fn scoped_search_limit_bounds_the_result_count() {
    let store = three_session_store();
    let hits = store
        .search_events_in_sessions("needle", &["s1", "s2"], 1)
        .unwrap();
    assert_eq!(hits.len(), 1, "limit applies to the scoped query too");
}

#[test]
fn unparseable_fts_syntax_is_a_typed_bad_query() {
    let store = seeded_store();
    seed(&store, "s1", &[user("plain text")]);

    for bad in ["\"unbalanced", "NEAR(", "AND"] {
        let err = store.search_events(bad, 10).unwrap_err();
        assert!(
            matches!(err, SessionError::BadQuery { .. }),
            "cross-session {bad:?} must be BadQuery, got {err:?}"
        );
        let err = store
            .search_events_in_sessions(bad, &["s1"], 10)
            .unwrap_err();
        assert!(
            matches!(err, SessionError::BadQuery { .. }),
            "scoped {bad:?} must be BadQuery, got {err:?}"
        );
    }
}
