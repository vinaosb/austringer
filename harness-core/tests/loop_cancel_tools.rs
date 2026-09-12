//! Cancel-during-operation, tool half (todo 5): ToolApproval and
//! ToolExecution probes — orphaned in-flight calls abort cleanly, leave no
//! half-state beyond the high-water mark, and execute exactly once on
//! resume.

mod common;

use std::time::Duration;

use common::TempSession;
use common::cancelkit::{cancel_config, cancelled, resumed_inference};
use common::loopkit::{call_turn, text_turn};
use common::stages_kit::*;
use harness_core::agent_loop::{AgentLoop, Operation, RunOutcome, SessionStatus};
use harness_core::event::Event;

#[tokio::test(start_paused = true)]
async fn cancel_mid_tool_approval() {
    let dir = TempSession::new("cancel-approval");
    let stages = stages(|s| s.approval = Box::new(CancelAwareApproval));
    // Seed one pending tool call so the approval stage actually runs:
    // first inference requests a call, then the next iteration parks in
    // approval.
    let first = call_turn("c1", "echo", serde_json::Value::Null);
    let second = text_turn("after tools");
    let inference = common::loopkit::ScriptedInference::new(vec![first, second]);
    let mut session = AgentLoop::open(dir.path(), cancel_config(), stages, Box::new(inference))
        .expect("open session");
    session.submit("use a tool");
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
    assert_eq!(record.operation, Operation::ToolApproval);
    assert_eq!(session.status(), SessionStatus::Cancelled);
    // The assistant turn + tool call ARE committed (complete operations);
    // no approval-denial events beyond the high-water mark.
    let events = session.events();
    assert_eq!(events.len(), record.high_water.0 as usize + 1);
    match &events.last().expect("mark").event {
        Event::System { text } => assert!(text.contains("ToolApproval")),
        other => panic!("mark expected, got {other:?}"),
    }
    // Resume: the orphan tool call is rebuilt from the log and executed.
    drop(session);
    let mut resumed = AgentLoop::open(
        dir.path(),
        harness_core::agent_loop::LoopConfig::default(),
        harness_core::agent_loop::StageSet::default(),
        Box::new(resumed_inference()),
    )
    .expect("reopen");
    resumed.submit("resume");
    assert_eq!(resumed.run().await, RunOutcome::TurnComplete);
    let tail = harness_core::log::replay(dir.path()).expect("replay");
    assert!(
        tail.events
            .iter()
            .any(|r| matches!(&r.event, Event::ToolResult { call_id, .. } if call_id == "c1"))
    );
    assert!(tail.anomalies.is_empty());
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_tool_execution() {
    let dir = TempSession::new("cancel-exec");
    let stages = stages(|s| s.execution = Box::new(CancelAwareExecution));
    let first = call_turn("c1", "long_tool", serde_json::Value::Null);
    let inference = common::loopkit::ScriptedInference::new(vec![first, text_turn("next")]);
    let mut session = AgentLoop::open(dir.path(), cancel_config(), stages, Box::new(inference))
        .expect("open session");
    session.submit("run the tool");
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
    assert_eq!(record.operation, Operation::ToolExecution);
    assert_eq!(session.status(), SessionStatus::Cancelled);
    // NO half-state: the in-flight execution wrote no ToolResult; the only
    // event beyond high water is the mark.
    let events = session.events();
    assert_eq!(events.len(), record.high_water.0 as usize + 1);
    assert!(
        !events
            .iter()
            .any(|r| matches!(&r.event, Event::ToolResult { call_id, .. } if call_id == "c1"))
    );
    // Resume-then-complete works; the orphan call executes exactly once.
    drop(session);
    let mut resumed = AgentLoop::open(
        dir.path(),
        harness_core::agent_loop::LoopConfig::default(),
        harness_core::agent_loop::StageSet::default(),
        Box::new(resumed_inference()),
    )
    .expect("reopen");
    resumed.submit("finish");
    assert_eq!(resumed.run().await, RunOutcome::TurnComplete);
    let tail = harness_core::log::replay(dir.path()).expect("replay");
    let results = tail
        .events
        .iter()
        .filter(|r| matches!(&r.event, Event::ToolResult { call_id, .. } if call_id == "c1"))
        .count();
    assert_eq!(results, 1, "orphan executed exactly once on resume");
    assert!(tail.anomalies.is_empty());
}
