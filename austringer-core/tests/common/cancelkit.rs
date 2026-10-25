//! Shared helpers for the cancel-probe test binaries (todo 5).

#![allow(dead_code)]

use std::time::Duration;

use super::TempSession;
use super::loopkit::text_turn;
use austringer_core::agent_loop::{
    AgentLoop, CancelRecord, LoopConfig, Operation, RunOutcome, SessionStatus, StageSet,
};
use austringer_core::event::Event;

/// Grace is generous: cancel probes exercise the CLEAN (cooperative)
/// path; forced-expiry lives in loop_grace.rs.
pub fn cancel_config() -> LoopConfig {
    LoopConfig {
        cancel_grace: Duration::from_secs(60),
        ..LoopConfig::default()
    }
}

/// Scripted inference for a probe run (never reached).
pub fn later_inference() -> super::loopkit::ScriptedInference {
    super::loopkit::ScriptedInference::new(vec![text_turn("later")])
}

/// Scripted inference for a resume run.
pub fn resumed_inference() -> super::loopkit::ScriptedInference {
    super::loopkit::ScriptedInference::new(vec![text_turn("resumed fine")])
}

/// Extracts the cancel record or panics with the actual outcome.
pub fn cancelled(outcome: RunOutcome) -> CancelRecord {
    match outcome {
        RunOutcome::Cancelled(record) => record,
        other => panic!("expected Cancelled, got {other:?}"),
    }
}

/// Reopens a cancelled session and completes one turn, asserting the
/// resume contract (InProgress, clean replay, log growth).
pub async fn assert_resumable(dir: &std::path::Path) {
    let mut resumed = AgentLoop::open(
        dir,
        LoopConfig::default(),
        StageSet::default(),
        Box::new(resumed_inference()),
    )
    .expect("reopen session");
    assert_eq!(resumed.status(), SessionStatus::InProgress);
    let before = resumed.events().len();
    resumed.submit("resume");
    assert_eq!(resumed.run().await, RunOutcome::TurnComplete);
    assert!(resumed.events().len() > before, "log grew on resume");
    let disk = austringer_core::log::replay(dir).expect("replay after");
    assert_eq!(disk.events.len(), resumed.events().len());
    assert!(disk.anomalies.is_empty());
}

/// Asserts the full clean-cancel contract at `operation`: unforced record,
/// Cancelled status, exactly one mark event beyond the high-water sequence
/// naming the operation, disk/memory agreement, and a resumable session.
pub async fn assert_clean_cancel_at(operation: Operation, stages: StageSet) {
    let dir = TempSession::new("cancel");
    let mut session = AgentLoop::open(
        dir.path(),
        cancel_config(),
        stages,
        Box::new(later_inference()),
    )
    .expect("open session");
    session.submit("work");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });

    let record = cancelled(outcome);
    assert!(!record.forced, "clean path expected for {operation:?}");
    assert_eq!(record.operation, operation);
    assert_eq!(session.status(), SessionStatus::Cancelled);
    assert_eq!(session.last_cancel(), Some(&record));

    // High-water honored: the ONLY event beyond the high-water sequence is
    // the cancel mark itself (seq high_water + 1), naming the operation.
    let events = session.events();
    assert_eq!(
        events.len(),
        record.high_water.0 as usize + 1,
        "exactly one mark event beyond high water"
    );
    let mark = events.last().expect("mark appended");
    assert_eq!(mark.seq.0, record.high_water.0 + 1);
    match &mark.event {
        Event::System { text } => {
            assert!(text.contains("SESSION CANCELLED"), "{text}");
            assert!(text.contains(operation.name()), "{text}");
        }
        other => panic!("mark must be a System event, got {other:?}"),
    }

    // Disk agrees with memory (assert content, not exit codes).
    let disk = austringer_core::log::replay(dir.path()).expect("replay");
    assert_eq!(disk.events.len(), events.len());
    assert!(disk.anomalies.is_empty());

    drop(session);
    assert_resumable(dir.path()).await;
}
