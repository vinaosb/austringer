//! Graceful-cancel window (todo 5, Warp semantics) under paused tokio time:
//! a cancel during the grace window reaches a clean boundary and lands
//! Cancelled cleanly; window expiry on a hung operation force-terminals
//! Cancelled without waiting forever.

mod common;

use std::time::Duration;

use austringer_core::agent_loop::{
    AgentLoop, LoopConfig, Operation, RunOutcome, SessionStatus, StageSet,
};
use austringer_core::event::Event;
use common::TempSession;
use common::stages_kit::{HungCompactor, SlowCompactor, stages};

#[tokio::test(start_paused = true)]
async fn cancel_during_grace_window_reaches_clean_boundary() {
    let dir = TempSession::new("grace-clean");
    // The in-flight op takes 100 ms (virtual); the window is 500 ms.
    let stages = stages(|s| s.compaction = Box::new(SlowCompactor));
    let config = LoopConfig {
        cancel_grace: Duration::from_millis(500),
        ..LoopConfig::default()
    };
    let inference =
        common::loopkit::ScriptedInference::new(vec![common::loopkit::text_turn("after compact")]);
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("work");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });

    let record = match outcome {
        RunOutcome::Cancelled(record) => record,
        other => panic!("expected Cancelled, got {other:?}"),
    };
    assert!(!record.forced, "stage finished inside the window");
    assert_eq!(record.operation, Operation::Compaction);
    assert_eq!(session.status(), SessionStatus::Cancelled);
}

#[tokio::test(start_paused = true)]
async fn grace_window_expiry_forces_terminal_cancelled() {
    let dir = TempSession::new("grace-forced");
    let stages = stages(|s| s.compaction = Box::new(HungCompactor));
    let config = LoopConfig {
        cancel_grace: Duration::from_millis(150),
        ..LoopConfig::default()
    };
    let inference =
        common::loopkit::ScriptedInference::new(vec![common::loopkit::text_turn("never")]);
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("work");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });

    let record = match outcome {
        RunOutcome::Cancelled(record) => record,
        other => panic!("expected Cancelled, got {other:?}"),
    };
    assert!(record.forced, "hung op must be force-terminalized");
    assert_eq!(record.operation, Operation::Compaction);
    assert_eq!(session.status(), SessionStatus::Cancelled);
    let events = session.events();
    assert_eq!(events.len(), record.high_water.0 as usize + 1);
    match &events.last().expect("mark").event {
        Event::System { text } => assert!(text.contains("forced"), "{text}"),
        other => panic!("mark expected, got {other:?}"),
    }
    // The session is still resumable after the forced terminalization.
    drop(session);
    let mut resumed = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        StageSet::default(),
        Box::new(common::loopkit::ScriptedInference::new(vec![
            common::loopkit::text_turn("resumed"),
        ])),
    )
    .expect("reopen");
    resumed.submit("again");
    assert_eq!(resumed.run().await, RunOutcome::TurnComplete);
    assert!(
        austringer_core::log::replay(dir.path())
            .expect("replay")
            .anomalies
            .is_empty()
    );
}

#[tokio::test(start_paused = true)]
async fn hung_inference_stream_is_force_cancelled_by_window_expiry() {
    let dir = TempSession::new("grace-infer");
    let config = LoopConfig {
        cancel_grace: Duration::from_millis(100),
        ..LoopConfig::default()
    };
    let mut session = AgentLoop::open(
        dir.path(),
        config,
        StageSet::default(),
        Box::new(common::stages_kit::HungInference),
    )
    .expect("open session");
    session.submit("stream");
    let handle = session.cancel_handle();
    let cancel_at = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        handle.cancel();
    };
    let (outcome, ()) = tokio::join!(session.run(), async {
        cancel_at.await;
    });
    let record = match outcome {
        RunOutcome::Cancelled(record) => record,
        other => panic!("expected Cancelled, got {other:?}"),
    };
    assert!(record.forced);
    assert_eq!(record.operation, Operation::Inference);
    assert_eq!(session.status(), SessionStatus::Cancelled);
    assert!(
        !session
            .events()
            .iter()
            .any(|r| matches!(r.event, Event::Assistant { .. }))
    );
}
