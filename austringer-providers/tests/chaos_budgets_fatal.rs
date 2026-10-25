//! Chaos suite, part 2: typed budget exhaustion (retry + resume), the
//! fatal error classes (identity mismatch, 4xx, in-stream errors, malformed
//! tool calls), and the retry-then-success path.

mod common;

use std::sync::Arc;

use austringer_providers::error::ProviderStreamError;
use austringer_providers::identity::ModelIdentity;
use austringer_test_support::provider::FakeApi;
use austringer_test_support::sse::{
    SseEvent, SseScript, ev_done, ev_message_start, ev_message_stop, ev_text_delta,
    ev_tool_call_delta,
};
use common::Plan;
use common::turnkit::{drive, reference_script, start};

#[tokio::test(start_paused = true)]
async fn resume_budget_exhaustion_is_typed_with_partial_delivery_count() {
    let script = reference_script();
    let offsets = common::turnkit::frame_offsets(&script);
    let k = (offsets[3] + 8) as u64;
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![
            Plan::dropped(script.clone(), k),
            Plan::dropped(script.clone(), k),
            Plan::dropped(script.clone(), k),
        ],
        2,
    );
    let (seen, envelope) = drive(&mut turn).await;
    match envelope.expect_err("budget must exhaust") {
        ProviderStreamError::ResumeExhausted {
            resumes,
            delivered_events,
            last,
        } => {
            assert_eq!(resumes, 2);
            assert!(delivered_events >= 2, "partial delivery counted");
            assert!(
                matches!(last.as_ref(), ProviderStreamError::Disconnected { .. }),
                "last failure is the disconnect"
            );
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert!(!seen.is_empty(), "events were delivered before exhaustion");
    assert_eq!(bridge.served().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn retry_budget_exhaustion_is_typed_before_any_delivery() {
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![
            Plan::server_error(),
            Plan::server_error(),
            Plan::server_error(),
        ],
        4,
    );
    let (seen, envelope) = drive(&mut turn).await;
    match envelope.expect_err("retry must exhaust") {
        ProviderStreamError::RetryExhausted { attempts, last } => {
            assert_eq!(attempts, 3);
            assert!(matches!(
                *last,
                ProviderStreamError::Http { status: 500, .. }
            ));
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert!(seen.is_empty(), "nothing delivered in the retry phase");
    assert_eq!(bridge.served().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn transient_server_error_retries_then_completes() {
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![
            Plan::server_error(),
            Plan::server_error(),
            Plan::ok(reference_script()),
        ],
        4,
    );
    let (_, mut reference_turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::ok(reference_script())],
        4,
    );
    let (reference_seen, reference_envelope) = drive(&mut reference_turn).await;

    let (seen, envelope) = drive(&mut turn).await;
    assert_eq!(
        envelope.expect("retry then success"),
        reference_envelope.expect("reference completes")
    );
    assert_eq!(seen, reference_seen);
    assert_eq!(bridge.served().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn identity_mismatch_is_fatal_and_never_reissued() {
    let script = SseScript::from_events([
        ev_message_start(),
        SseEvent::new(
            "text_delta",
            serde_json::json!({ "text": "hi", "model": "claude-4-opus" }),
        ),
        ev_message_stop(),
        ev_done(),
    ]);
    let (bridge, mut turn) = start(Arc::new(FakeApi::new()), vec![Plan::ok(script)], 4);
    let (seen, envelope) = drive(&mut turn).await;
    match envelope.expect_err("mismatch must fail the turn") {
        ProviderStreamError::IdentityMismatch {
            requested,
            received,
        } => {
            assert_eq!(requested, common::turnkit::identity());
            assert_eq!(received, ModelIdentity::new("anthropic", "claude-4-opus"));
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(seen.len(), 0, "mismatching event not delivered");
    assert_eq!(
        bridge.served().len(),
        1,
        "no re-issue on fatal identity error"
    );
}

#[tokio::test(start_paused = true)]
async fn non_recoverable_http_fails_immediately() {
    // No plans: the fake has no reply -> mapped HTTP 400 (non-retryable).
    let (bridge, mut turn) = start(Arc::new(FakeApi::new()), vec![], 4);
    let (_, envelope) = drive(&mut turn).await;
    match envelope.expect_err("4xx must surface") {
        ProviderStreamError::Http { status, code, .. } => {
            assert_eq!(status, 400);
            assert_eq!(code, "unexpected_request");
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(bridge.served().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn in_stream_error_event_surfaces_without_retry() {
    let script = SseScript::from_events([
        ev_message_start(),
        ev_text_delta("partial"),
        SseEvent::new(
            "error",
            serde_json::json!({ "code": "overloaded", "message": "upstream busy" }),
        ),
    ]);
    let (bridge, mut turn) = start(Arc::new(FakeApi::new()), vec![Plan::ok(script)], 4);
    let (seen, envelope) = drive(&mut turn).await;
    match envelope.expect_err("stream error surfaces") {
        ProviderStreamError::StreamError { code, .. } => assert_eq!(code, "overloaded"),
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(seen.len(), 1, "pre-error delta was delivered");
    assert_eq!(bridge.served().len(), 1, "stream errors are not retried");
}

#[tokio::test(start_paused = true)]
async fn invalid_tool_arguments_surface_typed_error() {
    let script = SseScript::from_events([
        ev_message_start(),
        SseEvent::new(
            "tool_call_start",
            serde_json::json!({ "call_id": "c1", "name": "bash" }),
        ),
        ev_tool_call_delta("c1", "{\"cmd\": "), // never completes valid JSON
        SseEvent::new("finish", serde_json::json!({ "reason": "tool_call" })),
        ev_done(),
    ]);
    let (_, mut turn) = start(Arc::new(FakeApi::new()), vec![Plan::ok(script)], 4);
    let (_, envelope) = drive(&mut turn).await;
    match envelope.expect_err("invalid args must fail the envelope") {
        ProviderStreamError::InvalidArguments { call_id } => assert_eq!(call_id, "c1"),
        other => panic!("wrong error: {other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn orphaned_tool_delta_without_start_surfaces_typed_error() {
    let script = SseScript::from_events([
        ev_message_start(),
        ev_tool_call_delta("ghost", "{}"),
        SseEvent::new("finish", serde_json::json!({ "reason": "tool_call" })),
        ev_done(),
    ]);
    let (_, mut turn) = start(Arc::new(FakeApi::new()), vec![Plan::ok(script)], 4);
    let (_, envelope) = drive(&mut turn).await;
    match envelope.expect_err("unnamed call must fail the envelope") {
        ProviderStreamError::UnnamedToolCall { call_id } => assert_eq!(call_id, "ghost"),
        other => panic!("wrong error: {other:?}"),
    }
}
