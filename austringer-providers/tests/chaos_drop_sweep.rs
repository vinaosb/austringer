//! Chaos suite, part 1: byte-K drop sweep across the whole stream (the
//! plan's acceptance probe), exactly-once tool execution, repeated
//! interruptions, and clean closes. Every probe asserts full event
//! sequences against a no-fault reference run.

mod common;

use std::sync::Arc;

use austringer_providers::event::ProviderEvent;
use austringer_test_support::provider::FakeApi;
use common::Plan;
use common::turnkit::{drive, frame_offsets, reference_script, start};

#[tokio::test(start_paused = true)]
async fn reference_run_shapes_the_expected_sequence() {
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::ok(reference_script())],
        4,
    );
    let (seen, envelope) = drive(&mut turn).await;
    let envelope = envelope.expect("clean run completes");

    assert_eq!(seen.len(), 6, "delivered sequence: {seen:?}");
    assert!(matches!(&seen[0], ProviderEvent::TextDelta { text, .. } if text == "Running grep"));
    assert!(
        matches!(&seen[1], ProviderEvent::ToolCallStart { call_id, name, .. } if call_id == "c1" && name == "bash")
    );
    assert_eq!(envelope.text, "Running grep");
    assert_eq!(envelope.tool_calls.len(), 1);
    assert_eq!(envelope.tool_calls[0].call_id, "c1");
    assert_eq!(
        envelope.tool_calls[0].arguments,
        serde_json::json!({ "cmd": "grep -r TODO src/" })
    );
    assert_eq!(
        envelope.finish_reason,
        austringer_providers::event::FinishReason::ToolCall
    );
    assert_eq!(envelope.usage.map(|u| u.input_tokens), Some(11));
    assert_eq!(bridge.served().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn drop_at_byte_k_across_the_stream_recovers_exactly_once() {
    let script = reference_script();
    let offsets = frame_offsets(&script);
    let total: usize = script.events().iter().map(|e| e.render().len()).sum();

    let (_, mut reference_turn) =
        start(Arc::new(FakeApi::new()), vec![Plan::ok(script.clone())], 4);
    let (reference_seen, reference_envelope) = drive(&mut reference_turn).await;
    let reference_envelope = reference_envelope.expect("reference completes");

    // K probes: mid-first-frame, mid-lead-text, at/before the call start,
    // INSIDE the args deltas (twice), after all args, mid-usage, mid-finish,
    // and the torn trailing sentinel. Each probe pairs with how many
    // requests the fake must see: 2 when recovery re-issues, 1 when the tear
    // lands after the finish event (turn already complete — no re-issue).
    let mut probes: Vec<(u64, usize)> = vec![(3, 2), (40, 2)];
    let call_start = 2; // msg_start, text, then tool_call_start frame
    probes.push((offsets[call_start] as u64, 2));
    probes.push((offsets[call_start] as u64 + 4, 2));
    for delta_index in [3, 4] {
        let len = script.events()[delta_index].render().len();
        probes.push(((offsets[delta_index] + len / 2) as u64, 2));
    }
    probes.push((offsets[5] as u64, 2));
    probes.push((offsets[6] as u64 + 3, 2));
    probes.push(((total - 1) as u64, 1));

    for (k, expected_served) in probes {
        let (bridge, mut turn) = start(
            Arc::new(FakeApi::new()),
            vec![Plan::dropped(script.clone(), k), Plan::ok(script.clone())],
            4,
        );
        let (seen, envelope) = drive(&mut turn).await;
        let envelope = envelope.unwrap_or_else(|err| panic!("K={k} must recover, got {err:?}"));
        assert_eq!(seen, reference_seen, "K={k} delivered sequence");
        assert_eq!(envelope, reference_envelope, "K={k} envelope");
        let served = bridge.served();
        assert_eq!(served.len(), expected_served, "K={k} request count");
        if expected_served == 2 {
            assert_eq!(
                served[0], served[1],
                "K={k} re-issue carries the identical request (stable turn id)"
            );
        }
    }
}

#[tokio::test(start_paused = true)]
async fn drop_mid_tool_args_resumed_turn_executes_the_tool_once() {
    // QA failure channel: the named probe from the acceptance criteria.
    let script = reference_script();
    let offsets = frame_offsets(&script);
    let args_delta = 4; // second tool_call_delta frame
    let len = script.events()[args_delta].render().len();
    let k = (offsets[args_delta] + len / 2) as u64;

    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::dropped(script.clone(), k), Plan::ok(script)],
        4,
    );
    let mut seen = Vec::new();
    while let Some(result) = turn.next_event().await {
        seen.push(result.expect("recovers"));
    }
    let envelope = turn.complete().await.expect("completes after resume");

    // The consumer executes the one complete call exactly once and
    // registers the side effect.
    let executed_once = envelope.tool_calls.len() == 1;
    assert!(executed_once, "exactly one complete call");
    turn.mark_executed("c1");
    let call = &envelope.tool_calls[0];
    assert_eq!(call.call_id, "c1");
    assert_eq!(
        call.arguments,
        serde_json::json!({ "cmd": "grep -r TODO src/" }),
        "args assembled whole across the tear"
    );
    let registry: Vec<_> = turn.dedup().executed().collect();
    assert_eq!(registry, vec!["c1"]);
    let starts = seen
        .iter()
        .filter(|e| matches!(e, ProviderEvent::ToolCallStart { .. }))
        .count();
    assert_eq!(starts, 1, "call start delivered exactly once");
    assert_eq!(bridge.served().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn drop_resume_drop_resume_drop_resume_still_exactly_once() {
    let script = reference_script();
    let offsets = frame_offsets(&script);
    let k1 = (offsets[2] + 4) as u64; // inside call start frame
    let k2 = (offsets[3] + 8) as u64; // inside first args delta
    let k3 = (offsets[5] + 2) as u64; // inside usage frame

    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![
            Plan::dropped(script.clone(), k1),
            Plan::dropped(script.clone(), k2),
            Plan::dropped(script.clone(), k3),
            Plan::ok(script.clone()),
        ],
        8,
    );
    let (_, mut reference_turn) =
        start(Arc::new(FakeApi::new()), vec![Plan::ok(script.clone())], 8);
    let (reference_seen, reference_envelope) = drive(&mut reference_turn).await;

    let (seen, envelope) = drive(&mut turn).await;
    assert_eq!(
        envelope.expect("completes after three interruptions"),
        reference_envelope.expect("reference completes")
    );
    assert_eq!(seen, reference_seen, "no duplicate events across 3 resumes");
    assert_eq!(bridge.served().len(), 4);
}

#[tokio::test(start_paused = true)]
async fn clean_close_without_finish_resumes_like_a_drop() {
    let script = reference_script();
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::closed(script.clone(), 3), Plan::ok(script)],
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
        envelope.expect("resumes after clean close"),
        reference_envelope.expect("ref")
    );
    assert_eq!(seen, reference_seen);
    assert_eq!(bridge.served().len(), 2);
}
