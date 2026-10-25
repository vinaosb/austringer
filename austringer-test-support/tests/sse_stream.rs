//! Self-test suite for stream-level injection (plan todo 2c): mid-stream drop
//! leaving a partial event stream, clean close-after-N, idle-timeout gaps, and
//! virtual-time delays. All timer-dependent tests run under `start_paused`, so
//! there are zero real sleeps anywhere in this suite.

use std::time::Duration;

use austringer_test_support::sse::{
    SseByteStream, SseParser, SseScriptBuilder, ev_done, ev_message_start, ev_text_delta,
};

fn three_event_script() -> austringer_test_support::sse::SseScript {
    SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("hello "))
        .event(ev_text_delta("world"))
        .event(ev_done())
        .build()
}

#[tokio::test(start_paused = true)]
async fn mid_stream_drop_leaves_partial_event_observable() {
    let script = three_event_script();
    let full: Vec<u8> = script
        .events()
        .iter()
        .flat_map(|e| e.render().into_bytes())
        .collect();

    // Cut 4 bytes into the last event's frame: the stream dies mid-event.
    let mut stream = SseByteStream::from_script(&script).drop_after_bytes((full.len() - 4) as u64);
    let received = stream.read_to_end().await;

    assert!(received.len() < full.len());
    let mut parser = SseParser::new();
    parser.feed(&received);
    assert_eq!(parser.events().len(), 3);
    assert!(parser.is_mid_event(), "torn final frame must be observable");
}

#[tokio::test(start_paused = true)]
async fn close_after_n_events_ends_cleanly_without_done() {
    let script = three_event_script();
    let mut stream = SseByteStream::from_script(&script).close_after_events(2);
    let received = stream.read_to_end().await;

    let mut parser = SseParser::new();
    parser.feed(&received);
    assert_eq!(parser.events().len(), 2);
    assert!(!parser.is_mid_event(), "clean close must not tear a frame");
    assert!(
        parser.events().iter().all(|e| e.event != "done"),
        "closed-before-done must be observable"
    );
}

#[tokio::test(start_paused = true)]
async fn idle_gap_triggers_consumer_timeout_under_paused_clock() {
    let script = SseScriptBuilder::new()
        .event(ev_message_start())
        .delay(Duration::from_secs(30))
        .event(ev_text_delta("late"))
        .build();
    let mut stream = SseByteStream::from_script(&script);

    let first = stream.next_chunk().await.expect("first chunk is immediate");
    let mut parser = SseParser::new();
    parser.feed(&first);
    assert_eq!(parser.events().len(), 1);

    let start = tokio::time::Instant::now();
    let second = tokio::time::timeout(Duration::from_secs(5), stream.next_chunk()).await;
    assert!(second.is_err(), "5s consumer timeout must beat the 30s gap");
    assert_eq!(start.elapsed(), Duration::from_secs(5));
}

#[tokio::test(start_paused = true)]
async fn inter_event_delays_advance_virtual_clock_exactly() {
    let script = SseScriptBuilder::new()
        .event(ev_message_start())
        .delay(Duration::from_secs(10))
        .event(ev_text_delta("a"))
        .delay(Duration::from_secs(10))
        .event(ev_text_delta("b"))
        .build();

    let start = tokio::time::Instant::now();
    let (text, events) = SseByteStream::from_script(&script).read_all().await;

    assert_eq!(text, "ab");
    assert_eq!(events.len(), 3);
    assert_eq!(start.elapsed(), Duration::from_secs(20));
}
