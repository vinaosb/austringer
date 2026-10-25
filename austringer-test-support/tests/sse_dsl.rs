//! Self-test suite for the SSE event-builder DSL (plan todo 2b):
//! `ev_*` constructors, script sequencing, `up_to_n_times`, delays-as-steps,
//! and the consumer-side parser's partial-frame observability.

use std::time::Duration;

use austringer_test_support::sse::{
    SseParser, SseScript, SseScriptBuilder, ev_done, ev_message_start, ev_message_stop,
    ev_text_delta, ev_usage,
};

#[test]
fn ev_constructors_render_standard_sse_wire_format() {
    let ev = ev_text_delta("hi");
    assert_eq!(ev.event, "text_delta");
    assert_eq!(ev.data["text"], "hi");
    let rendered = ev.render();
    assert!(rendered.starts_with("event: text_delta\ndata: {"));
    assert!(rendered.ends_with("\n\n"));
}

#[test]
fn single_text_script_has_start_delta_stop_done_shape() {
    let script = SseScript::single_text("hello");
    let names: Vec<String> = script.events().iter().map(|e| e.event.clone()).collect();
    assert_eq!(
        names,
        ["message_start", "text_delta", "message_stop", "done"]
    );
}

#[test]
fn builder_sequences_events_and_delays_in_order() {
    let script = SseScriptBuilder::new()
        .event(ev_message_start())
        .delay(Duration::from_millis(250))
        .event(ev_text_delta("a"))
        .event(ev_text_delta("b"))
        .event(ev_message_stop())
        .event(ev_usage(10, 4))
        .event(ev_done())
        .build();
    assert_eq!(script.steps.len(), 7);
    let events = script.events();
    assert_eq!(events.len(), 6);
    assert_eq!(events[1].data["text"], "a");
    assert_eq!(events[2].data["text"], "b");
}

#[test]
fn builder_up_to_n_times_repeats_steps_exactly_n_times() {
    let script = SseScriptBuilder::new()
        .event(ev_text_delta("x"))
        .up_to_n_times(3);
    let events = script.events();
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|e| e.data["text"] == "x"));

    let zero = SseScriptBuilder::new()
        .event(ev_text_delta("x"))
        .up_to_n_times(0);
    assert!(zero.events().is_empty());
}

#[test]
fn parser_reports_partial_event_after_incomplete_feed() {
    let full = SseScript::single_text("hello world");
    let bytes: Vec<u8> = full
        .events()
        .iter()
        .flat_map(|e| e.render().into_bytes())
        .collect();

    let mut parser = SseParser::new();
    let cut = bytes.len() - 5;
    parser.feed(&bytes[..cut]);

    assert_eq!(parser.events().len(), 3); // done event is torn away
    assert!(parser.is_mid_event());
    assert!(!parser.pending_fragment().is_empty());

    // Feeding the remainder completes the stream cleanly.
    parser.feed(&bytes[cut..]);
    assert_eq!(parser.events().len(), 4);
    assert!(!parser.is_mid_event());
}
