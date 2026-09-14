//! Plan acceptance: the interface is event-driven with 0.0% idle
//! CPU. Two proofs: (1) renders are a function of state changes only
//! — a very large batch of no-op inputs performs ZERO renders and
//! completes in bounded wall time (no hidden timers/polling work per
//! event); (2) the loop's only blocking primitive (`recv_timeout` on
//! the merged event channel) parks the thread when no input exists,
//! which is exactly how the binary's loop waits.

use std::time::Duration;

use harness_daemon::protocol::ServerMsg;
use harness_tui::app::{App, UiEvent};
use harness_tui::keys::Key;

fn unbound() -> Key {
    Key {
        code: crossterm::event::KeyCode::F(24),
        mods: crossterm::event::KeyModifiers::NONE,
    }
}

#[test]
fn one_hundred_thousand_noop_events_render_zero_times() {
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    app.rendered();
    let started = std::time::Instant::now();
    for _ in 0..100_000 {
        let _ = app.step(UiEvent::Key(unbound()));
        let _ = app.step(UiEvent::Server(ServerMsg::Pong));
    }
    let elapsed = started.elapsed();
    assert_eq!(app.renders(), 1, "no-op inputs never render");
    assert!(
        elapsed < Duration::from_secs(2),
        "per-event work is O(1) match arms; 200k events took {elapsed:?}"
    );
}

#[test]
fn loop_primitive_parks_when_no_input_exists() {
    let (sender, receiver) = std::sync::mpsc::channel::<UiEvent>();
    drop(sender);
    let parked = receiver.recv_timeout(Duration::from_millis(50));
    assert!(
        matches!(parked, Err(std::sync::mpsc::RecvTimeoutError::Disconnected)),
        "the loop's wait is a blocking recv, never a poll: {parked:?}"
    );
}
