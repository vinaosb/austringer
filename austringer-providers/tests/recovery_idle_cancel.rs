//! Idle-timeout, cancel/resume, and stale-state (snapshot round-trip)
//! recovery probes. All timing runs under the paused tokio clock.

mod common;

use std::sync::Arc;
use std::time::Duration;

use austringer_providers::error::ProviderStreamError;
use austringer_providers::event::ProviderEvent;
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::retry::{FixedJitter, JitterSource};
use austringer_providers::state::TurnSnapshot;
use austringer_providers::turn::{RecoveryConfig, ResumableTurn};
use austringer_test_support::gate::ResponseGate;
use austringer_test_support::provider::FakeApi;
use austringer_test_support::sse::{
    SseScript, SseScriptBuilder, ev_done, ev_message_start, ev_message_stop, ev_text_delta,
    ev_usage,
};
use common::{FakeBridge, Plan};

fn identity() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-4-5")
}

fn config_with_idle(idle: Duration, max_resumes: u32) -> RecoveryConfig {
    RecoveryConfig {
        retry: austringer_providers::retry::RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(4),
            multiplier: 2.0,
        },
        max_resumes,
        idle_timeout: idle,
    }
}

fn jitter() -> Box<dyn JitterSource> {
    Box::new(FixedJitter::new([0.5]))
}

fn plain_script() -> SseScript {
    SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("one "))
        .event(ev_text_delta("two "))
        .event(ev_text_delta("three"))
        .event(ev_usage(7, 5))
        .event(ev_message_stop())
        .event(ev_done())
        .build()
}

/// Script that stalls 30s (virtual) between the first and second deltas.
fn stalling_script() -> SseScript {
    SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("one "))
        .delay(Duration::from_secs(30))
        .event(ev_text_delta("two "))
        .event(ev_text_delta("three"))
        .event(ev_usage(7, 5))
        .event(ev_message_stop())
        .event(ev_done())
        .build()
}

fn start(
    api: Arc<FakeApi>,
    plans: Vec<Plan>,
    idle: Duration,
    max_resumes: u32,
) -> (FakeBridge, ResumableTurn) {
    let bridge = FakeBridge::new(api, identity(), plans);
    let turn = ResumableTurn::start(
        Arc::new(bridge.clone()),
        CompletionRequest::new(identity(), "hello"),
        config_with_idle(idle, max_resumes),
        jitter(),
    );
    (bridge, turn)
}

async fn collect(turn: &mut ResumableTurn) -> Vec<ProviderEvent> {
    let mut seen = Vec::new();
    while let Some(result) = turn.next_event().await {
        seen.push(result.expect("turn completes"));
    }
    seen
}

#[tokio::test(start_paused = true)]
async fn idle_timeout_fires_and_recovery_completes_the_turn() {
    // Budget 5s vs a 30s stall: the hung attempt is cut deterministically
    // under the paused clock and the resume (no stall) finishes the turn.
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::ok(stalling_script()), Plan::ok(plain_script())],
        Duration::from_secs(5),
        4,
    );
    let seen = collect(&mut turn).await;
    let envelope = turn.complete().await.expect("recovers after idle cut");
    assert_eq!(envelope.text, "one two three");
    assert_eq!(seen.len(), 5, "3 deltas + usage + finish: {seen:?}");
    assert_eq!(bridge.served().len(), 2, "hung attempt was re-issued");
}

#[tokio::test(start_paused = true)]
async fn permanently_hung_stream_surfaces_typed_idle_exhaustion() {
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![
            Plan::ok(stalling_script()),
            Plan::ok(stalling_script()),
            Plan::ok(stalling_script()),
        ],
        Duration::from_secs(5),
        2,
    );
    let mut terminal = None;
    let mut seen = Vec::new();
    while let Some(result) = turn.next_event().await {
        match result {
            Ok(event) => seen.push(event),
            Err(err) => {
                terminal = Some(err);
                break;
            }
        }
    }
    match terminal.expect("must exhaust") {
        ProviderStreamError::ResumeExhausted { resumes, last, .. } => {
            assert_eq!(resumes, 2);
            assert!(
                matches!(*last, ProviderStreamError::IdleTimedOut { budget } if budget == Duration::from_secs(5)),
                "last failure is the typed idle timeout"
            );
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(seen.len(), 1, "only the pre-stall delta was delivered");
    assert_eq!(bridge.served().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn abandoning_a_next_event_future_leaves_the_turn_resumable() {
    // Cancel/resume: a consumer that stops polling mid-event (dropped
    // future, the loop's cancel shape) finds the turn coherent and the
    // replay window still exactly-once.
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::ok(plain_script()), Plan::ok(plain_script())],
        Duration::from_secs(60),
        4,
    );

    // Consume two events normally.
    let first = turn.next_event().await.expect("delta").expect("ok");
    let second = turn.next_event().await.expect("delta").expect("ok");
    assert!(matches!(first, ProviderEvent::TextDelta { text, .. } if text == "one "));
    assert!(matches!(second, ProviderEvent::TextDelta { text, .. } if text == "two "));

    // "Cancel": race the next poll against a pre-released gate and drop the
    // poll future without driving it to completion.
    let gate = ResponseGate::new();
    gate.release();
    let raced = tokio::select! {
        biased;
        _ = gate.wait() => None,
        polled = turn.next_event() => Some(polled),
    };
    assert!(raced.is_none(), "cancel won the race");

    // Drop the whole turn (abandon), then rebuild from the snapshot and
    // finish on the second scripted reply.
    let snapshot = turn.snapshot();
    drop(turn);
    let snapshot: TurnSnapshot =
        serde_json::from_str(&serde_json::to_string(&snapshot).expect("serializes"))
            .expect("round-trips through JSON");
    let mut resumed = ResumableTurn::from_snapshot(
        Arc::new(bridge.clone()),
        snapshot,
        config_with_idle(Duration::from_secs(60), 4),
        jitter(),
    );
    let seen = collect(&mut resumed).await;
    let envelope = resumed.complete().await.expect("completes after restart");
    assert_eq!(envelope.text, "one two three");
    assert_eq!(
        seen.first(),
        Some(&ProviderEvent::TextDelta {
            model: identity(),
            text: "three".to_owned(),
        }),
        "resume continues AFTER the two already-delivered deltas"
    );
    assert_eq!(seen.len(), 3, "usage + finish follow: {seen:?}");
    assert_eq!(bridge.served().len(), 2, "resume re-issued once");
}

#[tokio::test(start_paused = true)]
async fn snapshot_preserves_registry_and_counters_across_restart() {
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::ok(plain_script()), Plan::ok(plain_script())],
        Duration::from_secs(60),
        4,
    );
    turn.next_event().await.expect("delta").expect("ok");
    turn.mark_executed("side-effect-observed-elsewhere");
    let snapshot = turn.snapshot();
    assert_eq!(
        snapshot.executed,
        vec!["side-effect-observed-elsewhere".to_owned()]
    );
    assert_eq!(snapshot.accepted, 1);

    let json = serde_json::to_string(&snapshot).expect("serializes");
    let restored: TurnSnapshot = serde_json::from_str(&json).expect("deserializes");
    let mut resumed = ResumableTurn::from_snapshot(
        Arc::new(bridge.clone()),
        restored,
        config_with_idle(Duration::from_secs(60), 4),
        jitter(),
    );
    let registry: Vec<_> = resumed.dedup().executed().collect();
    assert_eq!(registry, vec!["side-effect-observed-elsewhere"]);
    collect(&mut resumed).await;
    resumed.complete().await.expect("completes");
    assert_eq!(bridge.served().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn events_below_the_idle_budget_never_trip_the_timeout() {
    // Sub-budget gaps (100ms vs 5s) stream straight through: the idle
    // detector must not fire on healthy pacing.
    let paced = SseScriptBuilder::new()
        .event(ev_message_start())
        .event(ev_text_delta("a"))
        .delay(Duration::from_millis(100))
        .event(ev_text_delta("b"))
        .delay(Duration::from_millis(100))
        .event(ev_text_delta("c"))
        .event(ev_usage(1, 1))
        .event(ev_message_stop())
        .event(ev_done())
        .build();
    let (bridge, mut turn) = start(
        Arc::new(FakeApi::new()),
        vec![Plan::ok(paced)],
        Duration::from_secs(5),
        4,
    );
    let seen = collect(&mut turn).await;
    assert_eq!(seen.len(), 5);
    assert_eq!(bridge.served().len(), 1, "no spurious re-issue");
}
