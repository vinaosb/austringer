//! Inference-seam adapter tests, part 2: the todo-5 contract's hard edges —
//! cancel = abandon-not-drain (both pre-flight and mid-stream), typed
//! exhaustion surfaced as errors, and transparent mid-drop recovery inside
//! one `infer` call.

mod common;

use std::time::Duration;

use common::Plan;
use common::inferkit::{adapter, context};
use harness_core::agent_loop::{CancelToken, Inference, InferenceError, InferenceRequest};
use harness_test_support::sse::{
    SseScriptBuilder, ev_done, ev_message_start, ev_message_stop, ev_text_delta, ev_usage,
};

fn request<'a>(
    events: &'a [harness_core::event::RecordedEvent],
    focus: &'a str,
) -> InferenceRequest<'a> {
    InferenceRequest {
        events,
        focus_block: focus,
    }
}

fn stalling_script() -> harness_test_support::sse::SseScript {
    SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("partial"))
        .delay(Duration::from_secs(600))
        .event(ev_text_delta("never"))
        .event(ev_message_stop())
        .event(ev_done())
        .build()
}

#[tokio::test(start_paused = true)]
async fn cancellation_abandons_the_stream_and_errors() {
    // Pre-cancelled token: the adapter never even opens the stream (zero
    // requests — the cheapest possible abandonment).
    let (bridge, mut infer) = adapter(vec![Plan::ok(stalling_script())]);
    let cancel = CancelToken::new();
    cancel.cancel();
    let events = context();
    let result = infer.infer(&request(&events, "focus"), &cancel).await;
    let err = result.expect_err("cancelled turns error, never return partials");
    assert!(err.to_string().contains("abandoned"), "got: {err:?}");
    assert_eq!(bridge.served().len(), 0, "pre-cancelled: no request at all");

    // Mid-flight cancel: the request IS open and parked on the stall when
    // the token fires; the in-flight completion future is dropped (abandon,
    // never drained), one request served, nothing re-issued.
    let (bridge, infer2) = adapter(vec![Plan::ok(stalling_script())]);
    let cancel = CancelToken::new();
    let task_cancel = cancel.clone();
    let events = context();
    let handle = tokio::spawn(async move {
        let mut infer2 = infer2;
        infer2.infer(&request(&events, "focus"), &task_cancel).await
    });
    while bridge.served().is_empty() {
        tokio::task::yield_now().await;
    }
    cancel.cancel();
    let result = handle
        .await
        .expect("task joins")
        .expect_err("cancelled mid-flight");
    assert!(result.to_string().contains("abandoned"), "got: {result:?}");
    assert_eq!(
        bridge.served().len(),
        1,
        "no re-issue after mid-flight cancel"
    );
}

#[tokio::test(start_paused = true)]
async fn permanent_failure_is_an_error_never_a_partial_turn() {
    let (bridge, mut infer) = adapter(vec![
        Plan::server_error(),
        Plan::server_error(),
        Plan::server_error(),
    ]);
    let cancel = CancelToken::new();
    let events = context();
    let result = infer.infer(&request(&events, "focus"), &cancel).await;
    match result.expect_err("exhaustion errors") {
        InferenceError::Failure(message) => {
            assert!(message.contains("retry budget"), "got: {message}");
        }
    }
    assert_eq!(bridge.served().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn mid_stream_drop_recovers_inside_one_infer_call() {
    let script = SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("survives "))
        .event(ev_text_delta("disconnects"))
        .event(ev_usage(2, 2))
        .event(ev_message_stop())
        .event(ev_done())
        .build();
    let drop_after = (script.events()[2].render().len()
        + script.events()[1].render().len()
        + script.events()[0].render().len()) as u64;
    let (bridge, mut infer) = adapter(vec![
        Plan::dropped(script.clone(), drop_after + 2),
        Plan::ok(script),
    ]);
    let cancel = CancelToken::new();
    let events = context();
    let turn = infer
        .infer(&request(&events, "focus"), &cancel)
        .await
        .expect("recovery is transparent to the seam");
    assert_eq!(
        turn,
        harness_core::agent_loop::AssistantTurn {
            blocks: vec![harness_core::event::ContentBlock::Text {
                text: "survives disconnects".to_owned()
            }],
            tool_calls: Vec::new(),
        }
    );
    assert_eq!(
        bridge.served().len(),
        2,
        "one transparent resume inside infer"
    );
}
