//! Cancel-during-operation, first half of the pipeline (todo 5, THE core
//! adversarial class): EntryHook, SlashCommand, Steer, MaxTurns,
//! Compaction, ToolPairCompaction. Each probe cancels mid-stage and
//! asserts high-water, no-events-beyond-mark, resumability, no panic.

mod common;

use std::time::Duration;

use common::TempSession;
use common::cancelkit::{assert_clean_cancel_at, cancel_config, cancelled, later_inference};
use common::stages_kit::*;
use harness_core::agent_loop::{AgentLoop, Operation, SessionStatus};
use harness_core::event::Event;

#[tokio::test(start_paused = true)]
async fn cancel_mid_entry_hook() {
    let stages = stages(|s| s.entry_hook = Box::new(CancelAwareEntry));
    assert_clean_cancel_at(Operation::EntryHook, stages).await;
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_slash_command() {
    // The slash stage only runs when the queued input IS a command.
    let dir = TempSession::new("cancel-slash");
    let stages = stages(|s| s.slash_commands = Box::new(CancelAwareSlash));
    let mut session = AgentLoop::open(
        dir.path(),
        cancel_config(),
        stages,
        Box::new(later_inference()),
    )
    .expect("open session");
    session.submit("/plan hold");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });
    let record = cancelled(outcome);
    assert!(!record.forced);
    assert_eq!(record.operation, Operation::SlashCommand);
    assert_eq!(session.status(), SessionStatus::Cancelled);
    let events = session.events();
    assert_eq!(events.len(), record.high_water.0 as usize + 1);
    match &events.last().expect("mark").event {
        Event::System { text } => assert!(text.contains("SlashCommand")),
        other => panic!("mark expected, got {other:?}"),
    }
    drop(session);
    common::cancelkit::assert_resumable(dir.path()).await;
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_steer() {
    let stages = stages(|s| s.steer = Box::new(CancelAwareSteer));
    assert_clean_cancel_at(Operation::Steer, stages).await;
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_max_turns() {
    let stages = stages(|s| s.max_turns = Box::new(CancelAwareMaxTurns));
    assert_clean_cancel_at(Operation::MaxTurns, stages).await;
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_compaction() {
    let stages = stages(|s| s.compaction = Box::new(CancelAwareCompaction));
    assert_clean_cancel_at(Operation::Compaction, stages).await;
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_tool_pair_compaction() {
    let stages = stages(|s| s.tool_pair_compaction = Box::new(CancelAwarePairCompaction));
    assert_clean_cancel_at(Operation::ToolPairCompaction, stages).await;
}
