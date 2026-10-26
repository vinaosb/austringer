//! Pipeline policy behaviors (todo 5): stop-hook block cap, max-turns
//! gate, and inference retry budget.

mod common;

use austringer_core::agent_loop::{
    AgentLoop, FailureReason, LoopConfig, Operation, RunOutcome, SessionStatus, StageSet,
};
use austringer_core::event::{ContentBlock, Event, RecordedEvent};
use common::TempSession;
use common::loopkit::{ScriptedInference, call_turn, text_turn};
use common::stages_kit::{AlwaysBlockStopHook, BlockOnceStopHook, EndTurnOnAnyError, stages};

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

#[tokio::test]
async fn stop_hook_blocks_at_most_cap_times_then_must_let_exit_proceed() {
    let dir = TempSession::new("stopcap");
    let inference = ScriptedInference::new(vec![
        text_turn("answer 1"),
        text_turn("answer 2"),
        text_turn("answer 3"),
    ]);
    let stages = stages(|s| s.stop_hook = Box::new(AlwaysBlockStopHook));
    let config = LoopConfig {
        stop_hook_block_cap: 2,
        ..LoopConfig::default()
    };
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("go");

    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    // The hook wants to block forever; the cap forces exit after 2.
    let blocks = session
        .events()
        .iter()
        .filter(|r| matches!(&r.event, Event::System { text } if text.contains("keep going")))
        .count();
    assert_eq!(blocks, 2);
    let capped = session
        .events()
        .iter()
        .filter(|r| matches!(&r.event, Event::System { text } if text.contains("block cap")))
        .count();
    assert_eq!(capped, 1);
}

#[tokio::test]
async fn max_turns_exceeded_fails_the_session_typed() {
    let dir = TempSession::new("maxturns");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "echo", serde_json::Value::Null),
        call_turn("c2", "echo", serde_json::Value::Null),
        call_turn("c3", "echo", serde_json::Value::Null),
    ]);
    let config = LoopConfig {
        max_turns: 2,
        ..LoopConfig::default()
    };
    let mut session = AgentLoop::open(dir.path(), config, StageSet::default(), Box::new(inference))
        .expect("open session");
    session.submit("loop forever");
    assert_eq!(
        session.run().await,
        RunOutcome::Failed(FailureReason::MaxTurns { limit: 2 })
    );
    assert_eq!(session.status(), SessionStatus::Failed);
    // The pipeline really stopped at the gate: exactly 2 inferences ran
    // (assistant turns carrying only tool calls) and no results beyond.
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|r| matches!(r.event, Event::Assistant { .. }))
            .count(),
        2
    );
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|r| matches!(r.event, Event::ToolCall { .. }))
            .count(),
        2
    );
}

#[tokio::test]
async fn inference_failure_retries_once_then_fails_typed() {
    let dir = TempSession::new("retryfail");
    use austringer_core::agent_loop::InferenceError;
    let inference = ScriptedInference::from_results(vec![
        Err(InferenceError::Failure("boom".to_owned())),
        Err(InferenceError::Failure("boom".to_owned())),
    ]);
    let config = LoopConfig {
        max_inference_retries: 1,
        ..LoopConfig::default()
    };
    let mut session = AgentLoop::open(dir.path(), config, StageSet::default(), Box::new(inference))
        .expect("open session");
    session.submit("try");
    assert_eq!(
        session.run().await,
        RunOutcome::Failed(FailureReason::InferenceExhausted {
            cause: "boom".to_owned(),
            retries: 1,
        })
    );
    assert_eq!(session.status(), SessionStatus::Failed);
}

#[tokio::test]
async fn inference_failure_retry_then_success_completes_the_turn() {
    let dir = TempSession::new("retryok");
    use austringer_core::agent_loop::InferenceError;
    let inference = ScriptedInference::from_results(vec![
        Err(InferenceError::Failure("flake".to_owned())),
        Ok(text_turn("recovered")),
    ]);
    let config = LoopConfig {
        max_inference_retries: 1,
        ..LoopConfig::default()
    };
    let mut session = AgentLoop::open(dir.path(), config, StageSet::default(), Box::new(inference))
        .expect("open session");
    session.submit("try");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert_eq!(assistant_texts(session.events()), vec!["recovered"]);
}

#[tokio::test]
async fn turn_two_inference_failure_fails_typed_not_turn_complete() {
    // turn_ended stays true from the first clean turn; the stop-hook guard
    // must not consult over a pending inference error (swallow bug: the
    // failure surfaced as TurnComplete with zero failure events).
    let dir = TempSession::new("turn2fail");
    use austringer_core::agent_loop::InferenceError;
    let inference = ScriptedInference::from_results(vec![
        Ok(text_turn("first turn fine")),
        Err(InferenceError::Failure("boom".to_owned())),
        Err(InferenceError::Failure("boom".to_owned())),
    ]);
    let config = LoopConfig {
        max_inference_retries: 1,
        ..LoopConfig::default()
    };
    let mut session = AgentLoop::open(dir.path(), config, StageSet::default(), Box::new(inference))
        .expect("open session");
    session.submit("first");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    session.submit("second");
    assert_eq!(
        session.run().await,
        RunOutcome::Failed(FailureReason::InferenceExhausted {
            cause: "boom".to_owned(),
            retries: 1,
        })
    );
    assert_eq!(session.status(), SessionStatus::Failed);
    assert_eq!(assistant_texts(session.events()), vec!["first turn fine"]);
}

#[tokio::test]
async fn post_block_inference_failure_fails_typed_not_turn_complete() {
    // Within-run variant: the hook blocks once over a clean no-tool turn
    // (turn_ended=true), the demanded re-inference fails, retries exhaust —
    // the stale turn_ended must not let StopHook exit the turn first.
    let dir = TempSession::new("blockfail");
    use austringer_core::agent_loop::InferenceError;
    let inference = ScriptedInference::from_results(vec![
        Ok(text_turn("draft")),
        Err(InferenceError::Failure("boom".to_owned())),
        Err(InferenceError::Failure("boom".to_owned())),
    ]);
    let stages = stages(|s| s.stop_hook = Box::new(BlockOnceStopHook::new()));
    let config = LoopConfig {
        max_inference_retries: 1,
        ..LoopConfig::default()
    };
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("go");
    assert_eq!(
        session.run().await,
        RunOutcome::Failed(FailureReason::InferenceExhausted {
            cause: "boom".to_owned(),
            retries: 1,
        })
    );
    assert_eq!(session.status(), SessionStatus::Failed);
}

#[tokio::test]
async fn end_turn_flow_ends_quietly_without_failing_the_session() {
    let dir = TempSession::new("endturn");
    use austringer_core::agent_loop::InferenceError;
    let inference = ScriptedInference::from_results(vec![Err(InferenceError::Failure(
        "money budget exhausted".to_owned(),
    ))]);
    let stages = stages(|s| s.exit_on_error = Box::new(EndTurnOnAnyError));
    let config = LoopConfig {
        max_inference_retries: 0,
        ..LoopConfig::default()
    };
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert_eq!(session.status(), SessionStatus::InProgress);
}

#[test]
fn operation_pipeline_order_is_the_binding_sequence() {
    let expected = [
        "EntryHook",
        "SlashCommand",
        "Steer",
        "MaxTurns",
        "Compaction",
        "ToolPairCompaction",
        "ToolApproval",
        "ToolExecution",
        "Retry",
        "StopHook",
        "ExitOnError",
        "Inference",
    ];
    let names: Vec<&str> = Operation::ALL.iter().map(|op| op.name()).collect();
    assert_eq!(names, expected);
}
