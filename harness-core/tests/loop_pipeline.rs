//! Agent-loop pipeline behaviors (todo 5): scripted 3-turn conversation via
//! the FakeApi adapter, two-level completion with focus-chain blocking, and
//! the plan-gate hard-block.

mod common;

use std::sync::Arc;

use common::TempSession;
use common::loopkit::{FakeApiInference, ScriptedInference, call_turn, text_turn};
use harness_core::agent_loop::{
    AgentLoop, LoopConfig, RunOutcome, SessionStatus, StageSet, TASK_COMPLETED_TOOL,
};
use harness_core::event::{ContentBlock, Event, RecordedEvent};
use harness_test_support::provider::{FakeApi, RequestMatcher};
use harness_test_support::sse::SseScript;

fn assistant_texts(events: &[RecordedEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|recorded| match &recorded.event {
            Event::Assistant { blocks } => Some(
                blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.clone()),
                        ContentBlock::Image(_) | ContentBlock::Media(_) => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            _ => None,
        })
        .collect()
}

fn system_texts(events: &[RecordedEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|r| match &r.event {
            Event::System { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn three_turn_scripted_conversation_runs_the_full_pipeline() {
    let dir = TempSession::new("happy3");
    let api = Arc::new(FakeApi::new());
    api.on(RequestMatcher::Any)
        .reply_stream(SseScript::single_text("first answer"))
        .reply_stream(SseScript::single_text("second answer"))
        .reply_stream(SseScript::single_text("third answer"));
    let inference = FakeApiInference::new(api, "fake-model");
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(inference.clone()),
    )
    .expect("open session");

    // A seeded focus chain must ride along, unchanged, on every turn.
    session.focus_mut().upsert(1.into(), "keep context stable");

    for question in ["q1", "q2", "q3"] {
        session.submit(question);
        assert_eq!(session.run().await, RunOutcome::TurnComplete);
    }

    assert_eq!(session.status(), SessionStatus::InProgress);
    // Focus-chain re-injection stability: byte-identical block every turn.
    let seen = inference.seen();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].focus_block, seen[1].focus_block);
    assert_eq!(seen[1].focus_block, seen[2].focus_block);
    assert!(seen[0].focus_block.contains("<focus-chain>"));
    // Context accumulates: turn 3 sees turns 1 and 2.
    assert!(seen[2].context.contains("first answer"));
    assert!(seen[2].context.contains("q3"));
    // Event-log content, not just exit codes.
    assert_eq!(
        assistant_texts(session.events()),
        vec!["first answer", "second answer", "third answer"]
    );
    let disk = harness_core::log::replay(dir.path()).expect("replay");
    assert_eq!(disk.events.len(), 6);
    assert!(disk.anomalies.is_empty());
}

#[tokio::test]
async fn task_completion_is_blocked_while_todos_are_open_then_re_prompts() {
    let dir = TempSession::new("blocked");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", TASK_COMPLETED_TOOL, serde_json::Value::Null),
        text_turn("I still have todos open"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(inference),
    )
    .expect("open session");
    session.focus_mut().upsert(1.into(), "implement parser");
    session.focus_mut().upsert(2.into(), "write tests");
    session.submit("finish the task");

    // Turn 1: model tries task_completed with open todos => typed block,
    // re-prompt, then the model ends its turn (turn-level completion only).
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert_eq!(session.status(), SessionStatus::InProgress);
    let block = session
        .last_completion_block()
        .expect("typed blocked state recorded");
    assert_eq!(block.open_count(), 2);
    // The re-prompt carries the open chain.
    let system = system_texts(session.events());
    assert!(
        system
            .iter()
            .any(|t| t.contains("TASK COMPLETION BLOCKED") && t.contains("implement parser"))
    );
    let _ = block;

    // Close the todos; the next explicit completion succeeds.
    assert!(session.focus_mut().mark_done(1.into()));
    assert!(session.focus_mut().mark_done(2.into()));
    let inference2 = ScriptedInference::new(vec![call_turn(
        "c2",
        TASK_COMPLETED_TOOL,
        serde_json::Value::Null,
    )]);
    // Re-seat the inference seam on the live session.
    session.set_inference(Box::new(inference2));
    session.submit("finish now");
    assert_eq!(session.run().await, RunOutcome::TaskCompleted);
    assert_eq!(session.status(), SessionStatus::Success);
    assert_eq!(
        session.last_completion_block().map(|b| b.open_count()),
        None
    );
}

#[tokio::test]
async fn plan_gate_hard_block_defers_task_completion_until_cleared() {
    let dir = TempSession::new("plangate");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", TASK_COMPLETED_TOOL, serde_json::Value::Null),
        text_turn("plan still gated"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(inference),
    )
    .expect("open session");
    session.plan_gate().hard_block();
    session.submit("finish");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert_eq!(session.status(), SessionStatus::InProgress);
    assert!(matches!(
        session.last_completion_block(),
        Some(harness_core::agent_loop::CompletionBlock::PlanGate { .. })
    ));

    session.plan_gate().clear();
    let inference2 = ScriptedInference::new(vec![call_turn(
        "c2",
        TASK_COMPLETED_TOOL,
        serde_json::Value::Null,
    )]);
    session.set_inference(Box::new(inference2));
    session.submit("finish for real");
    assert_eq!(session.run().await, RunOutcome::TaskCompleted);
    assert_eq!(session.status(), SessionStatus::Success);
}
