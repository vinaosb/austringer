//! The externalized segment store: recoverable segment files + index,
//! persisted compaction state validated against a prefix hash, and the
//! folded-view reconstruction (plan todo 13, Grok/Cline patterns).

mod common;

use common::compactionkit::{facts_session, session_tool_returns};
use harness_core::compaction::overflow::{ContextOverflow, basic_compact};
use harness_core::compaction::segments::{
    FoldedView, PersistedCompaction, SegmentDetail, SegmentError, SegmentKind, SegmentStore,
};
use harness_core::compaction::{CompactedSession, StaleState};
use harness_core::event::{Event, RecordedEvent, Sequence};

fn compacted_fixture() -> CompactedSession {
    let events = facts_session();
    let table = session_tool_returns(&events);
    basic_compact(
        &events,
        &table,
        &Default::default(),
        &ContextOverflow::new(210_000, 200_000),
    )
}

/// What the store seals: the full stream minus the compacted
/// session's retained window (head + tail survive; the middle does
/// not).
fn sealed_of(session: &CompactedSession, events: &[RecordedEvent]) -> Vec<RecordedEvent> {
    let retained: std::collections::BTreeSet<u64> =
        session.retained.iter().map(|event| event.seq.0).collect();
    events
        .iter()
        .filter(|event| !retained.contains(&event.seq.0))
        .cloned()
        .collect()
}

#[test]
fn segments_and_index_land_under_session_compaction_dir() {
    let session = compacted_fixture();
    let events = facts_session();
    let _sealed = sealed_of(&session, &events);
    let store = SegmentStore::new(common::TempSession::new("segments-basic").path());
    assert_eq!(store.next_ordinal().expect("fresh store"), 1);
    let written = store
        .write_compaction(&session, &events, SegmentDetail::Standard)
        .expect("write succeeds");
    assert!(store.compaction_dir().is_dir(), "compaction dir exists");
    assert!(written.files.iter().all(|file| file.is_file()));
    assert!(written.index.is_file(), "index.json written");
    assert!(written.state.is_file(), "state.json written");
    assert!(
        written
            .files
            .iter()
            .all(|file| file.starts_with(store.compaction_dir()))
    );
    assert_eq!(store.next_ordinal().expect("ordinal advanced"), 2);
}

#[test]
fn detail_levels_control_segment_payload_mass() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let minimal = basic_compact(
        &events,
        &table,
        &Default::default(),
        &ContextOverflow::new(1, 1),
    );
    let _sealed = sealed_of(&minimal, &events);
    let mass = |detail: SegmentDetail| {
        let store = SegmentStore::new(common::TempSession::new("segments-detail").path());
        let session = minimal.clone();
        let written = store
            .write_compaction(&session, &events, detail)
            .expect("write");
        written
            .files
            .iter()
            .map(|file| std::fs::metadata(file).expect("stat").len())
            .sum::<u64>()
    };
    let minimal_mass = mass(SegmentDetail::Minimal);
    let standard_mass = mass(SegmentDetail::Standard);
    let full_mass = mass(SegmentDetail::Full);
    assert!(minimal_mass < standard_mass, "excerpts add mass");
    assert!(standard_mass < full_mass, "full events add more mass");
}

#[test]
fn loader_reconstructs_the_folded_view_from_segments() {
    let session = compacted_fixture();
    let events = facts_session();
    let sealed = sealed_of(&session, &events);
    let store = SegmentStore::new(common::TempSession::new("segments-reload").path());
    store
        .write_compaction(&session, &events, SegmentDetail::Full)
        .expect("write");
    let folded: FoldedView = store.load_folded().expect("reload succeeds");
    let rendered = folded.render();
    assert!(
        rendered.contains("the API port is 8085"),
        "reconstruction carries the doctrine summary"
    );
    assert!(
        rendered.contains("cache TTL is 90 seconds"),
        "reconstruction carries the retained span"
    );
    assert!(
        folded
            .segments
            .iter()
            .any(|record| matches!(record.kind, SegmentKind::Summary)),
        "a Summary segment exists"
    );
    assert!(
        folded
            .segments
            .iter()
            .any(|record| matches!(record.kind, SegmentKind::RetainedSpan)),
        "a RetainedSpan segment exists"
    );
    let full_sealed = folded
        .segments
        .iter()
        .find(|record| matches!(record.kind, SegmentKind::SealedSpan));
    assert!(
        full_sealed.is_some_and(|record| record.events.is_some()),
        "Full detail keeps the recoverable sealed events"
    );
    // The folded view is faithful: segment events reproduce the sealed
    // span exactly.
    let record = full_sealed
        .cloned()
        .unwrap_or_else(|| panic!("sealed record"));
    let recovered = record.events.unwrap_or_default();
    assert_eq!(
        recovered, sealed,
        "sealed events round-trip byte-identically"
    );
}

#[test]
fn torn_segment_file_is_a_typed_error() {
    let session = compacted_fixture();
    let events = facts_session();
    let _sealed = sealed_of(&session, &events);
    let store = SegmentStore::new(common::TempSession::new("segments-torn").path());
    let written = store
        .write_compaction(&session, &events, SegmentDetail::Full)
        .expect("write");
    // Tear one segment file: truncate mid-JSON.
    let victim = &written.files[0];
    let body = std::fs::read_to_string(victim).expect("read");
    std::fs::write(victim, &body[..body.len() / 2]).expect("tear");
    match store.load_folded() {
        Err(SegmentError::Torn { .. }) => {}
        other => panic!("expected Torn, got {other:?}"),
    }
}

#[test]
fn corrupt_index_is_a_typed_error() {
    let session = compacted_fixture();
    let events = facts_session();
    let _sealed = sealed_of(&session, &events);
    let store = SegmentStore::new(common::TempSession::new("segments-corrupt-index").path());
    let written = store
        .write_compaction(&session, &events, SegmentDetail::Minimal)
        .expect("write");
    std::fs::write(&written.index, b"{ not json").expect("corrupt");
    match store.load_folded() {
        Err(SegmentError::Corrupt { .. }) => {}
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

#[test]
fn missing_segments_directory_loads_as_empty_not_error() {
    let store = SegmentStore::new(common::TempSession::new("segments-empty").path());
    let folded = store.load_folded().expect("empty store loads");
    assert!(folded.segments.is_empty());
    assert_eq!(store.next_ordinal().expect("ordinal starts at 1"), 1);
}

#[test]
fn persisted_state_validates_against_the_prefix_hash() {
    let session = compacted_fixture();
    let events = facts_session();
    let sealed = sealed_of(&session, &events);
    let store = SegmentStore::new(common::TempSession::new("segments-state").path());
    store
        .write_compaction(&session, &events, SegmentDetail::Minimal)
        .expect("write");
    let state: PersistedCompaction = store
        .load_persisted_state()
        .expect("load")
        .expect("state present");
    assert_eq!(
        state.through_seq,
        sealed.last().map_or(Sequence(0), |event| event.seq),
        "the pinned prefix is the sealed extent, not the session watermark"
    );
    assert_eq!(state.before_tokens, session.before_tokens);

    // Honest resume: the SAME events validate.
    store
        .validate_prefix(&state, &events)
        .expect("identical events validate");

    // Stale state: events INSIDE the sealed prefix changed after the
    // compaction was saved — hash mismatch is typed, forcing a
    // re-compact.
    let mut drifted = events.clone();
    if let Event::User { blocks } = &mut drifted[0].event {
        blocks[0] = harness_core::event::content::ContentBlock::Text {
            text: "DECISION: the API port is 9999.".to_owned(),
        };
    }
    match store.validate_prefix(&state, &drifted) {
        Err(StaleState { .. }) => {}
        Ok(()) => panic!("mutated prefix must be stale"),
    }
}

#[test]
fn prefix_hash_is_order_and_content_sensitive() {
    let events = facts_session();
    let hash_a = harness_core::compaction::segments::sealed_prefix_hash(&events, Sequence(10));
    let hash_b = harness_core::compaction::segments::sealed_prefix_hash(&events, Sequence(11));
    assert_ne!(hash_a, hash_b, "the hash covers the exact prefix extent");
}
