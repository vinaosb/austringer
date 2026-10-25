//! Memory notes and hybrid retrieval: blend visibility, decay,
//! min-score gate, trigger depths.

use std::sync::Arc;

use austringer_sessions::{
    Clock, FakeLeaseProbe, InMemoryVectorIndex, NoteInput, RecallQuery, RecallTrigger, SessionStore,
};

fn clock_at(start: u64) -> Clock {
    let tick = Arc::new(std::sync::atomic::AtomicU64::new(start));
    Arc::new(move || tick.load(std::sync::atomic::Ordering::SeqCst))
}

#[test]
fn memory_add_and_recall_round_trip_with_visible_blend() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000_000), probe).unwrap();

    store
        .memory_add(NoteInput {
            session_id: None,
            title: "rust workspace layout".to_owned(),
            body: "the repo is a flat cargo workspace with 22 crates".to_owned(),
            vector: None,
        })
        .unwrap();
    store
        .memory_add(NoteInput {
            session_id: None,
            title: "baking".to_owned(),
            body: "sourdough needs a starter".to_owned(),
            vector: None,
        })
        .unwrap();

    let hits = store
        .recall(&RecallQuery {
            text: "workspace crates",
            vector: None,
            trigger: RecallTrigger::FirstTurn,
        })
        .unwrap();
    assert!(!hits.is_empty(), "expected at least one hit");
    assert_eq!(hits[0].note.title, "rust workspace layout");
    // Blend visible: the lexical component drove this ranking; the
    // vector component is 0.0 by construction without a vector index.
    assert!(hits[0].lexical > 0.0);
    assert_eq!(hits[0].vector, 0.0);
    let blended = 0.3 * hits[0].lexical;
    assert!((hits[0].blended - blended).abs() < 1e-9);
    assert!(hits[0].decayed <= hits[0].blended, "decay only reduces");
}

#[test]
fn hybrid_blend_prefers_vector_match_and_shows_both_halves() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000_000), probe).unwrap();
    let vectors = Arc::new(InMemoryVectorIndex::new());
    store.set_vector_index(Some(vectors.clone()));

    // Lexically identical strength, but only note A aligns with the
    // query vector — the 0.7 vector weight must break the tie.
    store
        .memory_add(NoteInput {
            session_id: None,
            title: "alpha note".to_owned(),
            body: "deployment pipeline notes".to_owned(),
            vector: Some(vec![1.0, 0.0]),
        })
        .unwrap();
    store
        .memory_add(NoteInput {
            session_id: None,
            title: "beta note".to_owned(),
            body: "deployment pipeline notes".to_owned(),
            vector: Some(vec![0.0, 1.0]),
        })
        .unwrap();

    let hits = store
        .recall(&RecallQuery {
            text: "deployment pipeline",
            vector: Some(&[0.9, 0.1]),
            trigger: RecallTrigger::FirstTurn,
        })
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(
        hits[0].note.title, "alpha note",
        "vector half must win the blend"
    );
    assert!(hits[0].vector > hits[1].vector);
    assert!((hits[0].lexical - hits[1].lexical).abs() < 1e-9);
    assert!(hits[0].blended > hits[1].blended);
    for hit in &hits {
        println!(
            "blend: {} lex={:.4} vec={:.4} blended={:.4} decayed={:.4}",
            hit.note.title, hit.lexical, hit.vector, hit.blended, hit.decayed
        );
    }
}

#[test]
fn temporal_decay_downweights_stale_notes() {
    let root = tempfile::tempdir().unwrap();
    let tick = Arc::new(std::sync::atomic::AtomicU64::new(1_000_000));
    let clock: Clock = {
        let tick = tick.clone();
        Arc::new(move || tick.load(std::sync::atomic::Ordering::SeqCst))
    };
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();

    store
        .memory_add(NoteInput {
            session_id: None,
            title: "fresh".to_owned(),
            body: "query tokens here".to_owned(),
            vector: None,
        })
        .unwrap();
    // 30 days pass; the same note ages.
    tick.store(1_000_000 + 30 * 86_400, std::sync::atomic::Ordering::SeqCst);
    let now_hits = store
        .recall(&RecallQuery {
            text: "query tokens",
            vector: None,
            trigger: RecallTrigger::FirstTurn,
        })
        .unwrap();
    // Single candidate: normalized lexical = 1.0, so blended = 0.3 and
    // decay applies exp(-0.05 * 30 days).
    let expected = 0.3 * (-0.05f64 * 30.0).exp();
    assert!(
        (now_hits[0].decayed - expected).abs() < 1e-9,
        "decay = 0.3 * exp(-0.05*30) = {expected}, got {}",
        now_hits[0].decayed
    );
    assert!(now_hits[0].final_score < 0.3);
}

#[test]
fn min_score_gate_drops_weak_hits() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000_000), probe).unwrap();
    store
        .memory_add(NoteInput {
            session_id: None,
            title: "unrelated".to_owned(),
            body: "zanzibar quixotic".to_owned(),
            vector: None,
        })
        .unwrap();
    let hits = store
        .recall(&RecallQuery {
            text: "completely different terms",
            vector: None,
            trigger: RecallTrigger::FirstTurn,
        })
        .unwrap();
    assert!(hits.is_empty(), "no lexical match must yield no hits");
}

#[test]
fn post_compaction_trigger_searches_deeper_than_first_turn() {
    let root = tempfile::tempdir().unwrap();
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock_at(1_000_000), probe).unwrap();
    for i in 0..12 {
        store
            .memory_add(NoteInput {
                session_id: None,
                title: format!("note {i}"),
                body: format!("shared topic number {i}"),
                vector: None,
            })
            .unwrap();
    }
    let first = store
        .recall(&RecallQuery {
            text: "shared topic",
            vector: None,
            trigger: RecallTrigger::FirstTurn,
        })
        .unwrap();
    let post = store
        .recall(&RecallQuery {
            text: "shared topic",
            vector: None,
            trigger: RecallTrigger::PostCompaction,
        })
        .unwrap();
    assert!(
        first.len() <= 8,
        "first turn is capped at 8, got {}",
        first.len()
    );
    assert!(
        post.len() > first.len(),
        "post-compaction re-search is deeper"
    );
}
