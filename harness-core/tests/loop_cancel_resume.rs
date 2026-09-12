//! Cancel/resume integrity (todo 5): repeated interruptions stay
//! consistent, and a clean cancel poisons no later prompt — the next
//! turn's context is exactly the committed prefix plus the mark.

mod common;

use std::time::Duration;

use common::TempSession;
use common::cancelkit::{cancel_config, resumed_inference};
use common::loopkit::{call_turn, text_turn};
use common::stages_kit::*;
use harness_core::agent_loop::{
    AgentLoop, CancelRecord, LoopConfig, RunOutcome, SessionStatus, StageSet,
};
use harness_core::event::Event;

#[tokio::test(start_paused = true)]
async fn repeated_interruptions_stay_consistent() {
    let dir = TempSession::new("repeat");
    // Interruption 1: cancel mid-compaction (after one steered turn).
    let stages1 = stages(|s| s.compaction = Box::new(CancelAwareCompaction));
    let inference = common::loopkit::ScriptedInference::new(vec![text_turn("t1")]);
    let mut session =
        AgentLoop::open(dir.path(), cancel_config(), stages1, Box::new(inference)).expect("open 1");
    session.submit("one");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });
    assert!(matches!(outcome, RunOutcome::Cancelled(_)));
    drop(session);

    // Interruption 2: one tool-call turn, cancel mid-approval.
    let stages2 = stages(|s| s.approval = Box::new(CancelAwareApproval));
    let call = call_turn("k1", "echo", serde_json::Value::Null);
    let inference = common::loopkit::ScriptedInference::new(vec![call, text_turn("t2")]);
    let mut session =
        AgentLoop::open(dir.path(), cancel_config(), stages2, Box::new(inference)).expect("open 2");
    session.submit("two");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });
    assert!(matches!(outcome, RunOutcome::Cancelled(_)));
    drop(session);

    // Final: complete a turn cleanly from the twice-interrupted log.
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(resumed_inference()),
    )
    .expect("open 3");
    session.submit("three");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let tail = harness_core::log::replay(dir.path()).expect("replay");
    assert!(tail.anomalies.is_empty());
    // Sequences strictly monotonic 1..=len.
    for (index, recorded) in tail.events.iter().enumerate() {
        assert_eq!(recorded.seq.0, (index + 1) as u64, "monotonic sequence");
    }
    // "one" and "two" were steered before their cancels; "three" now.
    let users = tail
        .events
        .iter()
        .filter(|r| matches!(r.event, Event::User { .. }))
        .count();
    assert_eq!(users, 3);
}

#[tokio::test(start_paused = true)]
async fn cancel_record_is_snapshot_only_no_events_poison_later_prompts() {
    // After a clean cancel the next turn's inference context is exactly
    // the committed prefix + mark: nothing half-written leaks in.
    let dir = TempSession::new("no-poison");
    let stages = stages(|s| s.execution = Box::new(CancelAwareExecution));
    let call = call_turn("p1", "slow_tool", serde_json::Value::Null);
    let inference = common::loopkit::ScriptedInference::new(vec![call]);
    let mut session =
        AgentLoop::open(dir.path(), cancel_config(), stages, Box::new(inference)).expect("open");
    session.submit("start");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });
    assert!(matches!(
        outcome,
        RunOutcome::Cancelled(CancelRecord { forced: false, .. })
    ));
    drop(session);

    let mut resumed = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(resumed_inference()),
    )
    .expect("reopen");
    assert_eq!(resumed.status(), SessionStatus::InProgress);
    resumed.submit("resume");
    assert_eq!(resumed.run().await, RunOutcome::TurnComplete);
}
