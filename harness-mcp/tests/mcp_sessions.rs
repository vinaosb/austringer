//! T29 server-mode session surface: the SessionControl seam (typed,
//! transport-agnostic; the daemon wires harness-sessions behind it — no
//! hard dependency, justified in the module docs), cost-limit
//! enforcement, and the GATHER fan-in barrier (await N session
//! completions, bounded).

use std::time::Duration;

use harness_core::budget::MicroUsd;
use harness_mcp::session_surface::{
    CostLimitOutcome, CreateRequest, EventChunk, GatherOutcome, GatherRegistry, InteractRequest,
    InteractionOutcome, ScriptedSessionControl, SessionControl, SessionId, SessionStatus,
    SurfaceError,
};

#[tokio::test]
async fn create_search_interact_round_trip_with_cost_limit() {
    let control = ScriptedSessionControl::default();
    let summary = control
        .create(CreateRequest {
            title: Some("demo".to_owned()),
        })
        .await
        .expect("create");
    assert_eq!(summary.title.as_deref(), Some("demo"));
    assert_eq!(summary.status, SessionStatus::Idle);

    let found = control.search("demo").await.expect("search");
    assert!(
        found
            .iter()
            .any(|item| item.title.as_deref() == Some("demo"))
    );

    let outcome = control
        .interact(InteractRequest {
            session: summary.id.clone(),
            prompt: "hello".to_owned(),
            cost_limit: Some(MicroUsd(1_000_000)),
        })
        .await
        .expect("interact");
    assert_eq!(outcome.reply, "scripted reply");
    assert_eq!(outcome.cost, CostLimitOutcome::WithinLimit);

    let events = control.events(&summary.id, 0).await.expect("events");
    assert!(!events.is_empty());
}

#[tokio::test]
async fn cost_limit_exceeded_is_typed() {
    let control = ScriptedSessionControl::default();
    let summary = control
        .create(CreateRequest { title: None })
        .await
        .expect("create");
    let denied = control
        .interact(InteractRequest {
            session: summary.id,
            prompt: "expensive please".to_owned(),
            // ScriptedSessionControl treats a zero limit as always-exceeded.
            cost_limit: Some(MicroUsd(0)),
        })
        .await;
    assert!(
        matches!(denied, Err(SurfaceError::CostLimitExceeded { .. })),
        "{denied:?}"
    );
}

#[tokio::test]
async fn unknown_session_is_typed_not_panicking() {
    let control = ScriptedSessionControl::default();
    let outcome = control
        .interact(InteractRequest {
            session: SessionId("nope".to_owned()),
            prompt: "hi".to_owned(),
            cost_limit: None,
        })
        .await;
    assert!(matches!(outcome, Err(SurfaceError::UnknownSession { .. })));
}

#[tokio::test]
async fn gather_awaits_n_completions() {
    let registry = GatherRegistry::new(8);
    let ids: Vec<SessionId> = (0..3).map(|index| SessionId(format!("s{index}"))).collect();
    let barrier = registry.barrier(ids.clone());
    for id in &ids[1..] {
        registry.complete(id.clone());
    }
    // Not yet complete: the barrier waits (bounded) for the missing one.
    registry.complete(ids[0].clone());
    let outcome = barrier.wait(Duration::from_secs(2)).await;
    assert_eq!(
        outcome,
        GatherOutcome::AllCompleted {
            completed: 3,
            timed_out: 0
        }
    );
}

#[tokio::test]
async fn gather_times_out_typed_with_partial_count() {
    let registry = GatherRegistry::new(8);
    let ids: Vec<SessionId> = (0..3).map(|index| SessionId(format!("s{index}"))).collect();
    let barrier = registry.barrier(ids.clone());
    registry.complete(ids[0].clone());
    let outcome = barrier.wait(Duration::from_millis(80)).await;
    assert!(
        matches!(
            outcome,
            GatherOutcome::Partial {
                completed: 1,
                timed_out: 2
            }
        ),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn gather_is_bounded_by_capacity() {
    let registry = GatherRegistry::new(2);
    let ids: Vec<SessionId> = (0..3).map(|index| SessionId(format!("s{index}"))).collect();
    let outcome = registry.try_barrier(ids);
    assert!(matches!(outcome, Err(SurfaceError::GatherUnbounded { .. })));
}

#[tokio::test]
async fn interaction_outcome_serializes_for_the_wire() {
    let outcome = InteractionOutcome {
        reply: "done".to_owned(),
        cost: CostLimitOutcome::WithinLimit,
    };
    let rendered = serde_json::to_string(&outcome).expect("serialize");
    assert!(rendered.contains("\"reply\":\"done\""));
    let chunk = EventChunk {
        ordinal: 7,
        kind: "assistant".to_owned(),
        text: "hi".to_owned(),
    };
    let rendered = serde_json::to_string(&chunk).expect("serialize");
    assert!(rendered.contains("\"ordinal\":7"));
}
