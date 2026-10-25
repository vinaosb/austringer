//! Chaos suite, part 3: the dedup registry across a DIVERGENT replay — the
//! no-duplicate-side-effects guarantee when the resumed stream re-emits an
//! already-executed tool call outside the identical-replay window.

mod common;

use std::sync::Arc;

use austringer_providers::event::ProviderEvent;
use austringer_test_support::provider::FakeApi;
use austringer_test_support::sse::{
    SseEvent, SseScriptBuilder, ev_done, ev_message_start, ev_text_delta, ev_tool_call_delta,
    ev_usage,
};
use common::Plan;
use common::turnkit::start;

#[tokio::test(start_paused = true)]
async fn already_executed_tool_call_is_filtered_on_resume() {
    // Attempt 1 delivers the full call args but no finish (stream ends
    // cleanly after the last delta). The consumer assembles the args,
    // EXECUTES, and marks the registry. The resume script diverges (new
    // text first, call re-emitted AFTER the replay window): re-delivered
    // call events must be filtered by the registry, not executed again.
    let first = SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("a"))
        .event(SseEvent::new(
            "tool_call_start",
            serde_json::json!({ "call_id": "c1", "name": "bash" }),
        ))
        .event(ev_tool_call_delta("c1", "{\"x\""))
        .event(ev_tool_call_delta("c1", ":1}"))
        .build();
    let resumed = SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("b1"))
        .event(ev_text_delta("b2"))
        .event(ev_text_delta("b3"))
        .event(ev_text_delta("b4"))
        .event(SseEvent::new(
            "tool_call_start",
            serde_json::json!({ "call_id": "c1", "name": "bash" }),
        ))
        .event(ev_tool_call_delta("c1", "{\"x\":1}"))
        .event(ev_usage(9, 9))
        .event(SseEvent::new(
            "finish",
            serde_json::json!({ "reason": "tool_call" }),
        ))
        .event(ev_done())
        .build();

    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::ok(first), Plan::ok(resumed)],
        4,
    );

    let mut seen = Vec::new();
    let mut executions = 0;
    let mut call_args = String::new();
    while let Some(result) = turn.next_event().await {
        let event = result.expect("recovers");
        match &event {
            ProviderEvent::ToolCallStart { call_id, .. } => {
                assert_eq!(call_id, "c1");
                call_args.clear();
                executions += 1;
            }
            ProviderEvent::ToolCallDelta {
                arguments_fragment, ..
            } => call_args.push_str(arguments_fragment),
            _ => {}
        }
        if matches!(event, ProviderEvent::ToolCallDelta { .. })
            && call_args.parse::<serde_json::Value>().is_ok()
        {
            // Args complete: execute ONCE and register the side effect.
            turn.mark_executed("c1");
        }
        seen.push(event);
    }
    let envelope = turn.complete().await.expect("completes");

    assert_eq!(
        executions, 1,
        "call start delivered to consumer exactly once"
    );
    let registry: Vec<_> = turn.dedup().executed().collect();
    assert_eq!(registry, vec!["c1"]);
    assert_eq!(
        envelope.tool_calls[0].arguments,
        serde_json::json!({ "x": 1 }),
        "envelope keeps the model-visible call exactly once"
    );
    let call_events = seen
        .iter()
        .filter(|e| {
            matches!(
                e,
                ProviderEvent::ToolCallStart { .. } | ProviderEvent::ToolCallDelta { .. }
            )
        })
        .count();
    assert_eq!(call_events, 3, "start + 2 deltas only: {seen:?}");
    assert_eq!(bridge.served().len(), 2);
}
