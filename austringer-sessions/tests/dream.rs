//! Gated /dream consolidation with an injected clock.

use std::sync::Arc;

use austringer_sessions::{
    Clock, DreamGates, DreamOutcome, FakeLeaseProbe, NoteInput, SessionStore,
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

#[test]
fn dream_gates_block_then_allow_with_injected_clock() {
    let root = tempfile::tempdir().unwrap();
    let (clock, tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    let gates = DreamGates {
        min_hours_since_last: 24,
        min_new_sessions: 2,
    };

    // Gates closed: no sessions yet.
    match store.consolidate_memory(&gates).unwrap() {
        DreamOutcome::NotYet(reason) => {
            assert_eq!(reason.sessions_remaining, 2);
            assert!(
                reason.hours_remaining == 0,
                "never-run passes the time gate"
            );
        }
        DreamOutcome::Consolidated { .. } => panic!("gates must block with zero sessions"),
    }

    store.create("s1", "").unwrap();
    store.create("s2", "").unwrap();
    // First run is allowed as soon as the session gate opens (the time
    // gate measures time since the LAST consolidation; never-run passes).
    advance(&tick, 1_000_000 + 3600);
    match store.consolidate_memory(&gates).unwrap() {
        DreamOutcome::Consolidated { merged } => assert!(merged.is_empty()),
        DreamOutcome::NotYet(reason) => panic!("first run must pass: {reason:?}"),
    }
    // Immediately after a run both gates close again.
    match store.consolidate_memory(&gates).unwrap() {
        DreamOutcome::NotYet(reason) => {
            assert_eq!(reason.hours_remaining, 24, "time gate re-arms at 24h");
            assert_eq!(reason.sessions_remaining, 2, "session counter reset");
        }
        DreamOutcome::Consolidated { .. } => panic!("gates must close after a run"),
    }
    // 26 hours after the run: the time gate opens; the session gate
    // needs two more sessions first.
    advance(&tick, 1_000_000 + 27 * 3600);
    match store.consolidate_memory(&gates).unwrap() {
        DreamOutcome::NotYet(reason) => assert_eq!(reason.sessions_remaining, 2),
        DreamOutcome::Consolidated { .. } => panic!("session gate must block"),
    }
    store.create("s3", "").unwrap();
    store.create("s4", "").unwrap();
    match store.consolidate_memory(&gates).unwrap() {
        DreamOutcome::Consolidated { .. } => {}
        DreamOutcome::NotYet(reason) => panic!("both gates open now: {reason:?}"),
    }
}

#[test]
fn dream_consolidation_merges_exact_duplicates_only() {
    let root = tempfile::tempdir().unwrap();
    let (clock, _tick) = clock_at(1_000_000);
    let probe = Arc::new(FakeLeaseProbe::new("boot-a"));
    let store = SessionStore::open_with(root.path(), clock, probe).unwrap();
    store.create("s1", "").unwrap();
    store.create("s2", "").unwrap();
    for _ in 0..3 {
        store
            .memory_add(NoteInput {
                session_id: None,
                title: "Same Title".to_owned(),
                body: "same body ".to_owned(),
                vector: None,
            })
            .unwrap();
    }
    store
        .memory_add(NoteInput {
            session_id: None,
            title: "Different".to_owned(),
            body: "unique".to_owned(),
            vector: None,
        })
        .unwrap();
    let outcome = store
        .consolidate_memory(&DreamGates {
            min_hours_since_last: 0,
            min_new_sessions: 2,
        })
        .unwrap();
    match outcome {
        DreamOutcome::Consolidated { merged } => {
            assert_eq!(merged.len(), 1, "exactly one duplicate group: {merged:?}");
            assert_eq!(merged[0].len(), 3);
        }
        DreamOutcome::NotYet(_) => panic!("zero-hour gates must pass"),
    }
    assert_eq!(store.memory_notes().unwrap().len(), 2);
}
