//! Cancel-during-operation, policy half (todo 5): Retry, StopHook,
//! ExitOnError, and a mid-stream Inference cancel (stream abandoned,
//! cancelled-at-seq mark).

mod common;

use std::sync::Arc;
use std::time::Duration;

use austringer_core::agent_loop::{
    AgentLoop, InferenceError, LoopConfig, Operation, SessionStatus, StageSet,
};
use austringer_core::event::Event;
use austringer_test_support::provider::{FakeApi, RequestMatcher};
use austringer_test_support::sse::{SseScriptBuilder, ev_text_delta};
use common::TempSession;
use common::cancelkit::{cancel_config, cancelled};
use common::loopkit::{FakeApiInference, text_turn};
use common::stages_kit::*;

#[tokio::test(start_paused = true)]
async fn cancel_mid_retry() {
    let dir = TempSession::new("cancel-retry");
    let stages = stages(|s| s.retry = Box::new(CancelAwareRetry));
    let inference = common::loopkit::ScriptedInference::from_results(vec![
        Err(InferenceError::Failure("first fails".to_owned())),
        Ok(text_turn("second works")),
    ]);
    let config = LoopConfig {
        max_inference_retries: 1,
        ..cancel_config()
    };
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("try twice");
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
    assert_eq!(record.operation, Operation::Retry);
    assert_eq!(session.status(), SessionStatus::Cancelled);
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_stop_hook() {
    let dir = TempSession::new("cancel-stophook");
    let stages = stages(|s| s.stop_hook = Box::new(CancelAwareStopHook));
    let inference = common::loopkit::ScriptedInference::new(vec![text_turn("done talking")]);
    let mut session = AgentLoop::open(dir.path(), cancel_config(), stages, Box::new(inference))
        .expect("open session");
    session.submit("talk");
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
    assert_eq!(record.operation, Operation::StopHook);
    assert_eq!(session.status(), SessionStatus::Cancelled);
}

#[tokio::test(start_paused = true)]
async fn cancel_mid_exit_on_error() {
    let dir = TempSession::new("cancel-exit");
    let stages = stages(|s| s.exit_on_error = Box::new(CancelAwareExitOnError));
    let inference = common::loopkit::ScriptedInference::from_results(vec![Err(
        InferenceError::Failure("fails".to_owned()),
    )]);
    let config = LoopConfig {
        max_inference_retries: 0,
        ..cancel_config()
    };
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("fail once");
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
    assert_eq!(record.operation, Operation::ExitOnError);
    assert_eq!(session.status(), SessionStatus::Cancelled);
}

#[tokio::test(start_paused = true)]
async fn cancel_while_inference_streams_abandons_and_marks_cancelled_at_seq() {
    let dir = TempSession::new("cancel-infer");
    let api = Arc::new(FakeApi::new());
    // A stream that stalls before any content: the cancel lands mid-stream.
    let stalled = SseScriptBuilder::new()
        .delay(Duration::from_secs(10))
        .event(ev_text_delta("too late"))
        .build();
    api.on(RequestMatcher::Any).reply_stream(stalled);
    let inference = FakeApiInference::new(api, "fake-model");
    let mut session = AgentLoop::open(
        dir.path(),
        cancel_config(),
        StageSet::default(),
        Box::new(inference),
    )
    .expect("open session");
    session.submit("stream slowly");
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
    assert_eq!(record.operation, Operation::Inference);
    assert_eq!(session.status(), SessionStatus::Cancelled);
    // The abandoned stream wrote no assistant event; only the mark exists
    // beyond the user turn.
    let events = session.events();
    assert_eq!(events.len(), record.high_water.0 as usize + 1);
    assert!(
        !events
            .iter()
            .any(|r| matches!(r.event, Event::Assistant { .. }))
    );
}
