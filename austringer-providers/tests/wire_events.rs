//! Wire-mapping tests: SSE event names + JSON payloads -> typed
//! `ProviderEvent`s, with malformed payloads skipped (never fatal) and the
//! identity stamping rule (wire `model` field wins over the configured one).

use austringer_providers::event::{FinishReason, ProviderEvent, Usage};
use austringer_providers::identity::ModelIdentity;
use austringer_providers::wire::{MappedEvent, map_wire_event};
use serde_json::json;

fn configured() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-4-5")
}

fn events(name: &str, data: serde_json::Value) -> Vec<MappedEvent> {
    map_wire_event(name, &data, &configured())
}

fn single(name: &str, data: serde_json::Value) -> MappedEvent {
    let mapped = events(name, data);
    assert_eq!(
        mapped.len(),
        1,
        "expected exactly one mapping for {name}: {mapped:?}"
    );
    mapped.into_iter().next().unwrap_or(MappedEvent::Skipped)
}

#[test]
fn text_delta_maps_with_configured_identity() {
    let MappedEvent::Event(ProviderEvent::TextDelta { model, text }) =
        single("text_delta", json!({ "text": "hello " }))
    else {
        panic!("expected TextDelta");
    };
    assert_eq!(text, "hello ");
    assert_eq!(model, configured());
}

#[test]
fn tool_call_fragments_carry_stable_call_ids() {
    let MappedEvent::Event(ProviderEvent::ToolCallStart {
        model,
        call_id,
        name,
    }) = single(
        "tool_call_start",
        json!({ "call_id": "c1", "name": "bash" }),
    )
    else {
        panic!("expected ToolCallStart");
    };
    assert_eq!((call_id.as_str(), name.as_str()), ("c1", "bash"));
    assert_eq!(model, configured());

    let MappedEvent::Event(ProviderEvent::ToolCallDelta {
        model,
        call_id,
        arguments_fragment,
    }) = single(
        "tool_call_delta",
        json!({ "call_id": "c1", "arguments": "{\"cmd\"" }),
    )
    else {
        panic!("expected ToolCallDelta");
    };
    assert_eq!(call_id.as_str(), "c1");
    assert_eq!(arguments_fragment, "{\"cmd\"");
    assert_eq!(model, configured());
}

#[test]
fn complete_tool_call_event_expands_to_start_plus_delta() {
    let mapped = events(
        "tool_call",
        json!({ "call_id": "c9", "name": "view_window", "arguments": "{\"path\":\"x\"}" }),
    );
    assert_eq!(mapped.len(), 2);
    assert!(matches!(
        &mapped[0],
        MappedEvent::Event(ProviderEvent::ToolCallStart { call_id, name, .. })
            if call_id == "c9" && name == "view_window"
    ));
    assert!(matches!(
        &mapped[1],
        MappedEvent::Event(ProviderEvent::ToolCallDelta { call_id, arguments_fragment, .. })
            if call_id == "c9" && arguments_fragment == "{\"path\":\"x\"}"
    ));
}

#[test]
fn usage_and_finish_vocabulary_map() {
    let MappedEvent::Event(ProviderEvent::Usage { usage, .. }) =
        single("usage", json!({ "input_tokens": 12, "output_tokens": 34 }))
    else {
        panic!("expected Usage");
    };
    assert_eq!(usage, Usage::new(12, 34));

    let MappedEvent::Event(ProviderEvent::Finish { reason, .. }) =
        single("message_stop", json!({ "type": "message_stop" }))
    else {
        panic!("expected Finish");
    };
    assert_eq!(reason, FinishReason::EndTurn);

    for (wire, expected) in [
        ("end_turn", FinishReason::EndTurn),
        ("length", FinishReason::Length),
        ("tool_call", FinishReason::ToolCall),
        ("refusal", FinishReason::Refusal),
    ] {
        let MappedEvent::Event(ProviderEvent::Finish { reason, .. }) =
            single("finish", json!({ "reason": wire }))
        else {
            panic!("expected Finish for {wire}");
        };
        assert_eq!(reason, expected, "reason vocabulary for {wire}");
    }
}

#[test]
fn wire_model_field_overrides_stamped_identity_for_mismatch_detection() {
    let MappedEvent::Event(ProviderEvent::TextDelta { model, .. }) = single(
        "text_delta",
        json!({ "text": "hi", "model": "claude-4-opus" }),
    ) else {
        panic!("expected TextDelta");
    };
    assert_eq!(model, ModelIdentity::new("anthropic", "claude-4-opus"));
}

#[test]
fn malformed_or_unknown_events_are_skipped_not_fatal() {
    // Payloads that do not fit the expected shape are skipped: the stream
    // continues (T2 parser already tolerates torn frames; this layer must not
    // turn a bad payload into a turn failure).
    for (name, data) in [
        ("text_delta", json!({ "no_text_field": true })),
        ("text_delta", json!({ "text": 42 })),
        ("tool_call_delta", json!({ "arguments": "x" })),
        ("tool_call_start", json!({ "call_id": "c1" })),
        ("tool_call", json!({ "call_id": "c1", "arguments": "{}" })),
        ("message_start", json!({ "role": "assistant" })),
        ("done", json!({ "type": "done" })),
        ("brand_new_event", json!({ "whatever": 1 })),
        ("finish", json!({ "reason": "quantum" })),
    ] {
        assert!(
            matches!(single(name, data), MappedEvent::Skipped),
            "{name} must map to Skipped"
        );
    }
}

#[test]
fn in_stream_error_event_maps_to_typed_error() {
    let MappedEvent::Error(err) = single("error", json!({ "code": "overloaded", "message": "m" }))
    else {
        panic!("typed error expected");
    };
    assert!(err.to_string().contains("overloaded"));
}
