//! Inference-seam adapter tests, part 1: the happy paths — llm-visible
//! projection into the prompt, internal-event exclusion, and tool-call
//! mapping into `PendingCall`s.

mod common;

use common::Plan;
use common::inferkit::{adapter, context, recorded};
use harness_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceRequest, TurnUsage,
};
use harness_core::event::{ContentBlock, Event, Sequence};
use harness_test_support::sse::{
    SseEvent, SseScript, SseScriptBuilder, ev_done, ev_message_start, ev_message_stop,
    ev_text_delta, ev_tool_call_delta, ev_usage,
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

#[tokio::test(start_paused = true)]
async fn text_turn_maps_through_the_llm_visible_projection() {
    let script = SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("found "))
        .event(ev_text_delta("three"))
        .event(ev_usage(3, 2))
        .event(ev_message_stop())
        .event(ev_done())
        .build();
    let (bridge, mut infer) = adapter(vec![Plan::ok(script)]);
    let cancel = CancelToken::new();
    let events = context();
    let turn = infer
        .infer(
            &request(&events, "<focus-chain>todos</focus-chain>"),
            &cancel,
        )
        .await
        .expect("completes");

    assert_eq!(
        turn,
        AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "found three".to_owned()
            }],
            tool_calls: Vec::new(),
            // ev_usage(3, 2) from the script rides the turn verbatim.
            usage: Some(TurnUsage {
                input_tokens: 3,
                output_tokens: 2,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            }),
        }
    );
    let served = bridge.served();
    assert_eq!(served.len(), 1);
    let prompt = &served[0].prompt;
    assert!(prompt.starts_with("<focus-chain>todos</focus-chain>\n"));
    assert!(prompt.contains("user: find the todos"));
    assert!(prompt.contains("assistant: on it"));
    assert!(prompt.contains("tool-call prev1 view_window"));
    assert!(prompt.contains("tool-result prev1"));
    assert!(prompt.contains("system: runtime notice"));
}

#[tokio::test(start_paused = true)]
async fn internal_events_never_reach_the_prompt() {
    let script = SseScript::single_text("ok");
    let (bridge, mut infer) = adapter(vec![Plan::ok(script)]);
    let cancel = CancelToken::new();
    let mut events = context();
    events.push(recorded(
        6,
        Event::Condensation {
            summary: "SECRET-COMPACTION-SUMMARY".to_owned(),
            through_seq: Sequence(6),
        },
    ));
    events.push(recorded(
        7,
        Event::Snapshot {
            label: "SECRET-SNAPSHOT-LABEL".to_owned(),
        },
    ));
    infer
        .infer(&request(&events, "focus"), &cancel)
        .await
        .expect("completes");
    let prompt = &bridge.served()[0].prompt;
    assert!(!prompt.contains("SECRET-COMPACTION-SUMMARY"));
    assert!(!prompt.contains("SECRET-SNAPSHOT-LABEL"));
    assert!(!prompt.contains("internal"));
}

#[tokio::test(start_paused = true)]
async fn tool_calls_map_to_pending_calls() {
    let script = SseScriptBuilder::new()
        .event(ev_message_start())
        .event(SseEvent::new(
            "tool_call_start",
            serde_json::json!({ "call_id": "c7", "name": "bash" }),
        ))
        .event(ev_tool_call_delta("c7", "{\"cmd\""))
        .event(ev_tool_call_delta("c7", ":\"ls\"}"))
        .event(ev_usage(5, 5))
        .event(SseEvent::new(
            "finish",
            serde_json::json!({ "reason": "tool_call" }),
        ))
        .event(ev_done())
        .build();
    let (_, mut infer) = adapter(vec![Plan::ok(script)]);
    let cancel = CancelToken::new();
    let events = context();
    let turn = infer
        .infer(&request(&events, "focus"), &cancel)
        .await
        .expect("completes");
    assert_eq!(
        turn.blocks,
        Vec::new(),
        "no text blocks in a pure tool turn"
    );
    assert_eq!(
        turn.tool_calls,
        vec![harness_core::agent_loop::PendingCall {
            call_id: "c7".to_owned(),
            tool: "bash".to_owned(),
            arguments: serde_json::json!({ "cmd": "ls" }),
        }]
    );
}
