//! Property tests for the event type system (plan todo 4, acceptance 1).
//!
//! Strategy: a dependency-free deterministic xorshift64* generator (fixed
//! seeds) produces randomized events across the full taxonomy; every event
//! must serialize -> deserialize to an equal value, and serializing twice
//! must yield byte-identical strings (field-order stability is what keeps
//! later prefix-cache layouts valid — the plan's "no unstable serialization"
//! guardrail).

mod common;

use common::{TempSession, XorShift64};
use harness_core::event::{
    ContentBlock, Event, EventClass, EventKind, RecordedEvent, Sequence, llm_visible,
};
use harness_core::log::{EventLogWriter, FsyncPolicy, replay};

const SEEDS: [u64; 4] = [1, 42, 0xDEAD_BEEF, 2026_0912];

#[test]
fn roundtrip_preserves_event_when_serialized_and_deserialized() {
    for &seed in &SEEDS {
        let mut rng = XorShift64::new(seed);
        for _ in 0..300 {
            let event = rng.event();
            let encoded = serde_json::to_string(&event).expect("serialize event");
            let decoded: Event = serde_json::from_str(&encoded).expect("deserialize event");
            assert_eq!(decoded, event, "seed {seed} round-trip diverged: {encoded}");
        }
    }
}

#[test]
fn serialization_is_stable_when_serializing_twice() {
    for &seed in &SEEDS {
        let mut rng = XorShift64::new(seed);
        for _ in 0..300 {
            let event = rng.event();
            let first = serde_json::to_string(&event).expect("first serialize");
            let second = serde_json::to_string(&event).expect("second serialize");
            assert_eq!(
                first, second,
                "seed {seed}: serialization is not byte-stable: {first}"
            );
        }
    }
}

#[test]
fn object_keys_are_sorted_when_maps_serialize() {
    // Guard against ever enabling serde_json's `preserve_order` feature:
    // with the default BTreeMap-backed Map, keys serialize sorted regardless
    // of insertion order.
    let mut map = serde_json::Map::new();
    map.insert("zebra".to_string(), serde_json::Value::from(1));
    map.insert("alpha".to_string(), serde_json::Value::from(2));
    map.insert("mid_keyword".to_string(), serde_json::Value::from(3));
    let event = Event::ToolCall {
        call_id: "call-1".into(),
        tool: "render".into(),
        arguments: serde_json::Value::Object(map),
    };
    let encoded = serde_json::to_string(&event).expect("serialize");
    let alpha = encoded.find("\"alpha\"").expect("alpha key present");
    let mid = encoded.find("\"mid_keyword\"").expect("mid key present");
    let zebra = encoded.find("\"zebra\"").expect("zebra key present");
    assert!(alpha < mid && mid < zebra, "keys not sorted in: {encoded}");
}

#[test]
fn recorded_sequence_roundtrips_when_written_and_replayed() {
    let session = TempSession::new("roundtrip-sequence");
    let mut rng = XorShift64::new(99);
    let mut writer =
        EventLogWriter::open(session.path(), FsyncPolicy::NeverForTests).expect("open writer");
    let originals: Vec<Event> = (0..500).map(|_| rng.event()).collect();
    for event in &originals {
        writer
            .append(event.clone())
            .expect("append within budget-free test policy");
    }
    drop(writer);

    let report = replay(session.path()).expect("replay session");
    assert!(
        report.anomalies.is_empty(),
        "unexpected anomalies: {:?}",
        report.anomalies
    );
    assert_eq!(report.events.len(), originals.len());
    for (index, recorded) in report.events.iter().enumerate() {
        assert_eq!(recorded.seq, Sequence(index as u64 + 1));
        assert_eq!(recorded.event, originals[index], "event {index} diverged");
    }
}

#[test]
fn visibility_classes_when_mapped_over_all_kinds() {
    let mut cases: Vec<(EventKind, EventClass)> = vec![
        (EventKind::User, EventClass::LlmVisible),
        (EventKind::Assistant, EventClass::LlmVisible),
        (EventKind::ToolCall, EventClass::LlmVisible),
        (EventKind::ToolResult, EventClass::LlmVisible),
        (EventKind::System, EventClass::LlmVisible),
        (EventKind::Condensation, EventClass::Internal),
        (EventKind::Snapshot, EventClass::Internal),
    ];
    cases.sort_by_key(|(kind, _)| format!("{kind:?}"));
    for (kind, class) in cases {
        assert_eq!(kind.class(), class, "{kind:?} has wrong class");
    }
}

#[test]
fn model_context_excludes_internal_events_when_filtered() {
    let mixed: Vec<RecordedEvent> = vec![
        RecordedEvent {
            seq: Sequence(1),
            event: Event::User {
                blocks: vec![ContentBlock::Text {
                    text: "look at this".into(),
                }],
            },
        },
        RecordedEvent {
            seq: Sequence(2),
            event: Event::Condensation {
                summary: "folded history".into(),
                through_seq: Sequence(1),
            },
        },
        RecordedEvent {
            seq: Sequence(3),
            event: Event::Assistant {
                blocks: vec![ContentBlock::Text {
                    text: "sure".into(),
                }],
            },
        },
        RecordedEvent {
            seq: Sequence(4),
            event: Event::Snapshot {
                label: "pre-edit".into(),
            },
        },
    ];
    let visible: Vec<&Event> = llm_visible(&mixed).collect();
    assert_eq!(
        visible.len(),
        2,
        "internal events leaked into model context"
    );
    for event in visible {
        assert_eq!(event.class(), EventClass::LlmVisible);
        // Belt and braces: no serialized model line may carry an internal tag.
        let encoded = serde_json::to_string(event).expect("serialize visible event");
        assert!(!encoded.contains("\"condensation\"") && !encoded.contains("\"snapshot\""));
    }
}
