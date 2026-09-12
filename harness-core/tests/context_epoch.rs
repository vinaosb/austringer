//! Context-epoch lifecycle and hold-effort-constant discipline (todo 11).
//!
//! The epoch is the immutable provider-cache baseline: frozen segment
//! hashes plus the pinned effort, surviving turn boundaries (history may
//! grow append-only) and ending ONLY at a completed compaction or session
//! movement, both explicit typed transitions. Mutating a frozen segment
//! mid-epoch is detected typed drift; changing effort mid-epoch is
//! rejected typed with the two sanctioned remedies (epoch boundary or
//! the per-message beta channel).

use harness_core::context::{
    ContextEpoch, ContextSpec, EffortChannel, EffortLevel, EffortSettings, EpochError, EpochId,
    EpochLedger, EpochTransition, PromptBuilder, RenderedPrompt, SegmentName, SlotSegment,
};
use harness_core::event::{ContentBlock, Event, RecordedEvent, Sequence};

fn medium() -> EffortSettings {
    EffortSettings {
        level: EffortLevel::Medium,
        thinking_budget: None,
    }
}

fn high() -> EffortSettings {
    EffortSettings {
        level: EffortLevel::High,
        thinking_budget: Some(1024),
    }
}

fn user_event(seq: u64, text: &str) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event: Event::User {
            blocks: vec![ContentBlock::Text {
                text: text.to_owned(),
            }],
        },
    }
}

fn spec_with(history: Vec<RecordedEvent>, system: &str, effort: EffortSettings) -> ContextSpec {
    ContextSpec {
        system: system.to_owned(),
        base_schema: "{\"core_tools\":[]}".to_owned(),
        rules: vec![SlotSegment {
            id: "focus".to_owned(),
            content: "<focus-chain>\nstatus: 0 open / 0 total\n</focus-chain>".to_owned(),
        }],
        skills: vec![],
        repomap: "map".to_owned(),
        history,
        last_user_message: "go".to_owned(),
        effort,
        beta_effort: None,
    }
}

/// Drives N turns against one epoch; returns (epoch, first render, last
/// render) so drift tests can mutate from a known-good baseline.
fn drive_turns(
    turns: u64,
    ledger: &mut EpochLedger,
) -> (ContextEpoch, RenderedPrompt, RenderedPrompt) {
    let mut history = vec![];
    let mut first: Option<RenderedPrompt> = None;
    let mut last: Option<RenderedPrompt> = None;
    let mut epoch: Option<ContextEpoch> = None;
    for turn in 1..=turns {
        let spec = spec_with(history.clone(), "agent", medium());
        let rendered = PromptBuilder::render(&spec).expect("render turn");
        if turn == 1 {
            epoch = Some(ledger.begin(&rendered, medium(), Sequence(2)));
            first = Some(rendered.clone());
        } else {
            epoch
                .as_ref()
                .expect("epoch begun")
                .check(&rendered, &medium())
                .expect("epoch intact");
        }
        history.push(user_event(turn * 2 - 1, &format!("u{turn}")));
        history.push(RecordedEvent {
            seq: Sequence(turn * 2),
            event: Event::Assistant {
                blocks: vec![ContentBlock::Text {
                    text: format!("a{turn}"),
                }],
            },
        });
        last = Some(rendered);
    }
    (
        epoch.expect("epoch begun"),
        first.expect("first render exists"),
        last.expect("last render exists"),
    )
}

#[test]
fn epoch_baseline_survives_turn_boundaries_while_history_grows() {
    let mut ledger = EpochLedger::new();
    let (epoch, first, last) = drive_turns(5, &mut ledger);
    assert_eq!(epoch.id, EpochId(1));
    assert_eq!(ledger.epochs().len(), 1);
    // Frozen segment hashes pinned at begin() still match after growth.
    assert_eq!(epoch.baseline.segment_hashes, first.frozen_hashes());
    assert_eq!(epoch.started_at_seq, Sequence(2));
    // History grew, so the full stable-prefix hash moved, but the epoch
    // stayed: growth is append-only, never drift.
    assert_ne!(
        first.stable_prefix_hash, last.stable_prefix_hash,
        "history growth should have moved the prefix hash"
    );
    assert_eq!(ledger.transitions().len(), 0);
    assert_eq!(ledger.current().expect("begun").id, EpochId(1));
}

#[test]
fn mid_epoch_frozen_mutation_is_drift_detected_typed() {
    let mut ledger = EpochLedger::new();
    let (epoch, _, _) = drive_turns(3, &mut ledger);
    // A durable update landing in the system segment mid-epoch.
    let drifted = PromptBuilder::render(&spec_with(vec![], "agent v2", medium())).expect("render");
    match epoch.check(&drifted, &medium()) {
        Err(EpochError::Drift(drift)) => {
            assert_eq!(drift.segment, SegmentName::System);
            assert_eq!(
                drift.expected,
                epoch
                    .baseline_hash(SegmentName::System)
                    .expect("baseline hash")
            );
            assert_ne!(drift.expected, drift.actual);
            assert!(!drift.to_string().is_empty());
        }
        other => panic!("expected Drift, got {other:?}"),
    }
    // Repomap drift behaves identically.
    let mut spec = spec_with(vec![], "agent", medium());
    spec.repomap = "map v2".to_owned();
    let drifted = PromptBuilder::render(&spec).expect("render");
    assert!(matches!(
        epoch.check(&drifted, &medium()),
        Err(EpochError::Drift(drift)) if drift.segment == SegmentName::Repomap
    ));
}

#[test]
fn compaction_transition_flips_epoch_deterministically() {
    let mut ledger = EpochLedger::new();
    let (old_epoch, _, _) = drive_turns(4, &mut ledger);
    // Compaction rewrote history into a summary; the post-compaction
    // render becomes the new baseline, and effort may move with it.
    let compacted = PromptBuilder::render(&spec_with(
        vec![user_event(1, "condensed: everything before turn 4")],
        "agent",
        high(),
    ))
    .expect("render compacted");
    let new_epoch = ledger.transition(
        EpochTransition::CompactionCompleted {
            through_seq: Sequence(8),
        },
        &compacted,
        high(),
    );
    assert_eq!(new_epoch.id, EpochId(2));
    assert_ne!(old_epoch.id, new_epoch.id);
    assert_eq!(new_epoch.started_at_seq, Sequence(9));
    assert_eq!(new_epoch.baseline.segment_hashes, compacted.frozen_hashes());
    assert_eq!(new_epoch.effort, high());
    // The transition is an explicit recorded event.
    assert_eq!(ledger.transitions().len(), 1);
    let record = &ledger.transitions()[0];
    assert_eq!(record.from, EpochId(1));
    assert_eq!(record.to, EpochId(2));
    assert!(matches!(
        record.transition,
        EpochTransition::CompactionCompleted {
            through_seq: Sequence(8)
        }
    ));
    // The new epoch holds across subsequent turns (stale-state guard).
    let grown = PromptBuilder::render(&spec_with(
        vec![
            user_event(1, "condensed"),
            user_event(9, "post-compaction turn"),
        ],
        "agent",
        high(),
    ))
    .expect("render grown");
    new_epoch.check(&grown, &high()).expect("new epoch holds");
}

#[test]
fn session_movement_ends_the_epoch_too() {
    let mut ledger = EpochLedger::new();
    let _ = drive_turns(2, &mut ledger);
    let rendered = PromptBuilder::render(&spec_with(vec![], "agent", medium())).expect("render");
    let moved = ledger.transition(
        EpochTransition::SessionMovement {
            moved_at_seq: Sequence(4),
        },
        &rendered,
        medium(),
    );
    assert_eq!(moved.id, EpochId(2));
    assert_eq!(moved.started_at_seq, Sequence(5));
}

#[test]
fn mid_epoch_effort_change_is_rejected_typed_with_both_remedies() {
    let mut ledger = EpochLedger::new();
    let (epoch, _, _) = drive_turns(2, &mut ledger);
    let requested = EffortSettings {
        level: EffortLevel::High,
        thinking_budget: None,
    };
    let high_render =
        PromptBuilder::render(&spec_with(vec![], "agent", requested.clone())).expect("render");
    match epoch.check(&high_render, &requested) {
        Err(EpochError::Effort(violation)) => {
            assert_eq!(violation.epoch, EpochId(1));
            assert_eq!(violation.current, medium());
            assert_eq!(violation.requested, requested);
            assert_eq!(
                violation.remedies,
                &[EffortChannel::EpochBoundary, EffortChannel::PerMessageBeta]
            );
            assert!(!violation.to_string().is_empty());
        }
        other => panic!("expected Effort rejection, got {other:?}"),
    }
    // The beta channel is the sanctioned mid-epoch path: metadata only,
    // never rendered bytes, so the epoch check stays green.
    let mut spec = spec_with(vec![], "agent", medium());
    spec.beta_effort = Some(requested.clone());
    let beta_render = PromptBuilder::render(&spec).expect("render beta");
    epoch
        .check(&beta_render, &medium())
        .expect("beta channel never violates the epoch");
    assert_eq!(beta_render.beta_effort, Some(requested));
}

#[test]
fn qa_prints_epoch_rejection_payloads() {
    let mut ledger = EpochLedger::new();
    let (epoch, _, _) = drive_turns(2, &mut ledger);
    println!("=== FAILURE QA: mid-epoch rejections (typed, with guidance) ===");
    let requested = EffortSettings {
        level: EffortLevel::High,
        thinking_budget: None,
    };
    let high_render =
        PromptBuilder::render(&spec_with(vec![], "agent", requested.clone())).expect("render");
    let Err(EpochError::Effort(violation)) = epoch.check(&high_render, &requested) else {
        panic!("expected effort rejection");
    };
    println!("  [effort/mid-epoch] -> {violation}");
    let drifted = PromptBuilder::render(&spec_with(vec![], "agent v2", medium())).expect("render");
    let Err(EpochError::Drift(drift)) = epoch.check(&drifted, &medium()) else {
        panic!("expected drift");
    };
    println!("  [drift/system-mutation] -> {drift}");
}

#[test]
fn ledger_serializes_stably_for_the_session_store() {
    let mut ledger = EpochLedger::new();
    let _ = drive_turns(2, &mut ledger);
    let rendered = PromptBuilder::render(&spec_with(vec![], "agent", medium())).expect("render");
    ledger.transition(
        EpochTransition::SessionMovement {
            moved_at_seq: Sequence(4),
        },
        &rendered,
        medium(),
    );
    let first = serde_json::to_string(&ledger).expect("serialize ledger");
    let second = serde_json::to_string(&ledger).expect("serialize again");
    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "ledger serialization must be byte-stable"
    );
    let restored: EpochLedger = serde_json::from_str(&first).expect("restore ledger");
    assert_eq!(restored.current().expect("begun").id, EpochId(2));
    assert_eq!(restored.epochs().len(), 2);
    assert_eq!(restored.transitions().len(), 1);
    assert_eq!(EpochLedger::new().epochs().len(), 0);
}
