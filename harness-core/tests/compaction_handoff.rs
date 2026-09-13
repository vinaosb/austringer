//! Handoff complement and teamwork successor-handoff (plan todo 13,
//! Amp/Antigravity patterns): a distilled transfer bundle produced at
//! compaction time, read_thread-style extraction over referenced
//! sessions through an injected second-model seam, and the typed
//! successor package for milestone boundaries.

mod common;

use std::sync::Arc;

use common::compactionkit::{FailingExtractor, MockExtractor, facts_session, session_tool_returns};
use harness_core::compaction::condense::CondenseError;
use harness_core::compaction::handoff::{
    HandoffBundle, MilestoneMarker, SuccessorBundle, latest_milestone, read_thread,
};
use harness_core::compaction::overflow::{ContextOverflow, basic_compact};
use harness_core::event::{Event, RecordedEvent, Sequence};

#[test]
fn handoff_bundle_from_compaction_carries_summary_and_pointers() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let compacted = basic_compact(
        &events,
        &table,
        &Default::default(),
        &ContextOverflow::new(1, 1),
    );
    let bundle = HandoffBundle::from_compaction(&compacted);
    assert!(
        bundle.distilled_summary.contains("the API port is 8085"),
        "the distilled transfer carries the doctrine summary"
    );
    assert!(
        !bundle.salient_pointers.is_empty(),
        "salient pointers reference the old session"
    );
    assert!(
        bundle
            .salient_pointers
            .iter()
            .all(|pointer| pointer.seq <= compacted.through_seq),
        "pointers stay inside the compacted session"
    );
}

#[tokio::test]
async fn read_thread_extracts_through_the_injected_seam() {
    let events = facts_session();
    let bundle = read_thread(&MockExtractor, &events)
        .await
        .expect("mock extractor succeeds");
    assert!(bundle.distilled_summary.contains("36 events"));
    assert_eq!(bundle.salient_pointers.len(), 1);
}

#[tokio::test]
async fn read_thread_error_is_typed() {
    let events = facts_session();
    let err = read_thread(&FailingExtractor, &events)
        .await
        .expect_err("failing extractor surfaces the seam error");
    assert!(err.to_string().contains("extractor offline"));
}

#[test]
fn milestone_markers_round_trip_from_events() {
    let marker = MilestoneMarker {
        label: "billing-mvp".to_owned(),
    };
    assert_eq!(marker.render(), "<milestone label=\"billing-mvp\"/>");
    let parsed = MilestoneMarker::parse(&marker.render()).expect("parses back");
    assert_eq!(parsed, marker);
    assert!(MilestoneMarker::parse("<milestone>").is_err());
    assert!(MilestoneMarker::parse("not a milestone").is_err());

    let events = facts_session();
    let latest = latest_milestone(&events).expect("fixture carries one milestone");
    assert_eq!(latest, marker);
}

#[test]
fn successor_bundle_is_a_typed_milestone_package() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let compacted = basic_compact(
        &events,
        &table,
        &Default::default(),
        &ContextOverflow::new(1, 1),
    );
    let handoff = HandoffBundle::from_compaction(&compacted);
    let focus = "<focus-chain>\nstatus: 2 open / 3 total\n</focus-chain>";
    let successor = SuccessorBundle::at_milestone(
        MilestoneMarker {
            label: "billing-mvp".to_owned(),
        },
        handoff,
        focus.to_owned(),
        vec!["wire checkout".to_owned(), "harden cache".to_owned()],
    );
    assert_eq!(successor.milestone.label, "billing-mvp");
    assert!(
        successor.handoff.distilled_summary.contains("Postgres 15"),
        "the successor inherits the distilled context"
    );
    assert_eq!(successor.open_todos.len(), 2);
    assert_eq!(successor.focus_chain, focus);
}

#[test]
fn latest_milestone_ignores_plain_system_notices() {
    let noise: Vec<RecordedEvent> = vec![
        RecordedEvent {
            seq: Sequence(1),
            event: Event::System {
                text: "EVENT LOG DEGRADED — session is now READ-ONLY".to_owned(),
            },
        },
        RecordedEvent {
            seq: Sequence(2),
            event: Event::System {
                text: "<milestone label=\"first-green\"/>".to_owned(),
            },
        },
    ];
    let latest = latest_milestone(&noise).expect("milestone found past the noise");
    assert_eq!(latest.label, "first-green");
    let no_milestone: Vec<RecordedEvent> = vec![noise[0].clone()];
    assert!(latest_milestone(&no_milestone).is_none());
}

#[allow(dead_code)]
fn seam_error_type_is_the_shared_condense_error() {
    let _: CondenseError = CondenseError("x".to_owned());
    let _: Option<Arc<MockExtractor>> = None;
}
