//! The dispatcher (plan todo 42): every turn resolves under the routed
//! profile (asserted on the OBSERVED dispatch — identity + knobs — never a
//! config echo), per-task profile stickiness pins the launch profile
//! across resume and subtask inheritance, and every failure path ASKS
//! before substituting — outage proposes an acknowledged fallback, refusal
//! and truncation retry with an attempt counter, rate limits wait out a
//! bounded window before failing over, and the verbatim chain exhausts
//! into a typed failure.

use harness_intel::catalog::{Catalog, EffortValue};
use harness_intel::routing::dispatch::Dispatcher;
use harness_intel::routing::fallback::{
    DispatchFailure, FallbackDecision, RoutingFailure, StopReason, TurnOutcome,
};
use harness_intel::routing::resolve::{PinConflict, TaskId};
use harness_intel::routing::{Role, RouteRequest, RoutingTable, Tier};
use harness_providers::identity::ModelIdentity;

const VALID: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["gpt-5-5", "deepseek-reasoner"],
  "roles": [
    { "role": "architect",  "model": "claude-opus-4-7" },
    { "role": "editor",     "model": "deepseek-reasoner" },
    { "role": "commit-message", "model": "gpt-5-5", "effort": "none" }
  ],
  "tiers": [
    { "tier": "light",  "model": "gpt-5-5",           "effort": "low", "score_floor": 0.60 },
    { "tier": "medium", "model": "claude-sonnet-4-6",                  "score_floor": 0.80 }
  ],
  "fallbacks": [
    { "model": "claude-opus-4-7", "chain": ["claude-opus-5", "claude-sonnet-4-6"] },
    { "model": "gpt-5-5",         "chain": ["claude-sonnet-4-6"] }
  ],
  "rate_limit_wait_out_secs": 45,
  "retry_attempts": 2,
  "benchmarks": [
    { "model": "claude-sonnet-4-6", "score": 0.85, "price_per_mtok": 3.0 },
    { "model": "gpt-5-5",           "score": 0.70, "price_per_mtok": 1.2 }
  ]
}
"#;

fn dispatcher() -> Dispatcher {
    let table = RoutingTable::from_json(VALID, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("valid config rejected: {e:?}"));
    Dispatcher::new(table)
}

fn outage() -> TurnOutcome {
    TurnOutcome::Failed {
        failure: DispatchFailure::Outage,
    }
}

fn refused() -> TurnOutcome {
    TurnOutcome::Completed {
        stop: StopReason::RefusalTruncation,
    }
}

#[test]
fn every_turn_dispatches_under_the_routed_profile() {
    // Given: a dispatcher over the role config.
    let mut d = dispatcher();
    let task = TaskId::new("t1");
    // When: three turns dispatch under the architect role.
    for _ in 0..3 {
        let dispatch = d
            .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
            .expect("dispatch");
        // Then: the OBSERVED request carries the routed identity + knob.
        assert_eq!(
            dispatch.model,
            ModelIdentity::new("anthropic", "claude-opus-4-7")
        );
        assert_eq!(dispatch.knobs.effort, Some(EffortValue::Xhigh));
        assert!(!dispatch.auto_routed);
        assert!(dispatch.fallback.is_none());
    }
    // And utility roles dispatch on their cheap models.
    let util = TaskId::new("t-util");
    let dispatch = d
        .dispatch_turn(&util, RouteRequest::Role(Role::CommitMessage))
        .expect("dispatch");
    assert_eq!(dispatch.model, ModelIdentity::new("anthropic", "gpt-5-5"));
    assert_eq!(dispatch.knobs.effort, Some(EffortValue::None));
}

#[test]
fn launch_profile_sticks_across_resume_and_subtask_inheritance() {
    // Given: a task launched under the medium tier.
    let mut d = dispatcher();
    let task = TaskId::new("launch");
    let first = d
        .dispatch_turn(&task, RouteRequest::Tier(Tier::Medium))
        .expect("dispatch");
    assert_eq!(first.model.model, "claude-sonnet-4-6");
    assert!(first.auto_routed);
    // When: the task resumes under a DIFFERENT explicit request.
    let resumed = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    // Then: the pin wins — no silent model drift mid-task.
    assert_eq!(resumed.model.model, "claude-sonnet-4-6");
    assert_eq!(resumed.effort, EffortValue::Medium);
    // And a subtask inherits the parent's launch profile.
    let child = TaskId::new("child");
    d.spawn_subtask(&task, child.clone()).expect("inherit");
    let sub = d
        .dispatch_turn(&child, RouteRequest::Role(Role::Editor))
        .expect("dispatch");
    assert_eq!(sub.model.model, "claude-sonnet-4-6");
    // But an explicit unpin allows a deliberate re-route.
    assert!(d.unpin(&task));
    let rerouted = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    assert_eq!(rerouted.model.model, "claude-opus-4-7");
}

#[test]
fn pin_conflicts_fail_closed_at_the_stickiness_layer() {
    use harness_intel::routing::resolve::Stickiness;
    // Given: a pinned task.
    let table = RoutingTable::from_json(VALID, &Catalog::seeded()).expect("table");
    let mut sticky = Stickiness::new();
    let task = TaskId::new("t");
    let profile = table
        .resolve(RouteRequest::Role(Role::Architect))
        .expect("res");
    sticky
        .pin(
            task.clone(),
            harness_intel::routing::resolve::PinnedProfile {
                request: RouteRequest::Role(Role::Architect),
                resolution: profile,
            },
        )
        .expect("pin");
    // When: a DIFFERENT profile re-pins without unpinning.
    let other = table
        .resolve(RouteRequest::Role(Role::Editor))
        .expect("res");
    let conflict = sticky
        .pin(
            task.clone(),
            harness_intel::routing::resolve::PinnedProfile {
                request: RouteRequest::Role(Role::Editor),
                resolution: other,
            },
        )
        .unwrap_err();
    // Then: typed conflict naming both models.
    let PinConflict {
        task: t,
        pinned_model,
        requested_model,
    } = conflict;
    assert_eq!(t, task);
    assert_eq!(pinned_model, "claude-opus-4-7");
    assert_eq!(requested_model, "deepseek-reasoner");
    // And subtasks of unknown parents are typed misses.
    assert!(
        sticky
            .spawn_subtask(&TaskId::new("ghost"), TaskId::new("c"))
            .is_err()
    );
}

#[test]
fn outage_asks_never_substitutes_until_acknowledged() {
    // Given: a dispatched architect turn on opus-4-7 (chain: opus-5, sonnet).
    let mut d = dispatcher();
    let task = TaskId::new("t");
    let dispatch = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    let original = dispatch.model.clone();
    // When: the turn observes an outage.
    let ask = match d.observe_turn(&task, &original, outage()) {
        FallbackDecision::AskFallback(fr) => fr,
        other => panic!("expected ask, got {other:?}"),
    };
    // Then: the FIRST verbatim hop is proposed with a surfaced reason.
    assert_eq!(ask.from, original);
    assert_eq!(ask.to, ModelIdentity::new("anthropic", "claude-opus-5"));
    assert!(!ask.reason.is_empty());
    // And the very next dispatch STILL targets the original model — no
    // silent substitution happens anywhere.
    let unchanged = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    assert_eq!(unchanged.model, original);
    assert!(unchanged.fallback.is_none());
    // And a mismatched acknowledgement is rejected typed.
    let wrong = harness_intel::routing::FallbackRequest::new(
        original.clone(),
        ModelIdentity::new("anthropic", "claude-sonnet-4-6"),
        "wrong hop",
    )
    .acknowledge();
    assert!(d.acknowledge_fallback(&task, wrong).is_err());
    // But the honest acknowledgement switches the wire w/ audit trail.
    let ack = ask.acknowledge();
    d.acknowledge_fallback(&task, ack.clone()).expect("ack");
    let switched = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    assert_eq!(
        switched.model,
        ModelIdentity::new("anthropic", "claude-opus-5")
    );
    let trail = switched.fallback.as_ref().expect("audit trail");
    assert_eq!(
        trail.to(),
        &ModelIdentity::new("anthropic", "claude-opus-5")
    );
    assert_eq!(trail.from(), &original);
    // Effort stays explicit for the hop model (xhigh is on opus-5's grid).
    assert_eq!(switched.knobs.effort, Some(EffortValue::Xhigh));
}

#[test]
fn verbatim_chain_walks_in_order_and_exhausts_typed() {
    let mut d = dispatcher();
    let task = TaskId::new("t");
    let first = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    // Hop 1: opus-4-7 -> opus-5 (chain position 0, verbatim first).
    let ask1 = match d.observe_turn(&task, &first.model, outage()) {
        FallbackDecision::AskFallback(fr) => fr,
        other => panic!("expected ask, got {other:?}"),
    };
    assert_eq!(ask1.to.model, "claude-opus-5");
    d.acknowledge_fallback(&task, ask1.acknowledge())
        .expect("ack");
    let second = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    // Hop 2: opus-5 outage -> sonnet (chain position 1 — NEVER round-robin).
    let ask2 = match d.observe_turn(&task, &second.model, outage()) {
        FallbackDecision::AskFallback(fr) => fr,
        other => panic!("expected ask, got {other:?}"),
    };
    assert_eq!(ask2.to.model, "claude-sonnet-4-6");
    d.acknowledge_fallback(&task, ask2.acknowledge())
        .expect("ack");
    let third = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    assert_eq!(third.model.model, "claude-sonnet-4-6");
    // Chain exhausted: the NEXT outage is a typed failure, never a guess.
    match d.observe_turn(&task, &third.model, outage()) {
        FallbackDecision::Fail(RoutingFailure::ChainExhausted { origin, chain }) => {
            assert_eq!(origin, "claude-opus-4-7");
            assert_eq!(chain.len(), 2);
        }
        other => panic!("expected exhausted, got {other:?}"),
    }
    // And a model with NO chain fails typed immediately.
    let lonely = TaskId::new("lonely");
    let dispatch = d
        .dispatch_turn(&lonely, RouteRequest::Role(Role::Editor))
        .expect("dispatch");
    match d.observe_turn(&lonely, &dispatch.model, outage()) {
        FallbackDecision::Fail(RoutingFailure::NoFallbackConfigured { model }) => {
            assert_eq!(model, "deepseek-reasoner");
        }
        other => panic!("expected no-chain, got {other:?}"),
    }
}

#[test]
fn declining_a_proposal_fails_typed_and_keeps_the_original() {
    let mut d = dispatcher();
    let task = TaskId::new("t");
    let dispatch = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    let original = dispatch.model.clone();
    match d.observe_turn(&task, &original, outage()) {
        FallbackDecision::AskFallback(_) => {}
        other => panic!("expected ask, got {other:?}"),
    }
    match d.decline_fallback(&task) {
        RoutingFailure::Declined { from, to } => {
            assert_eq!(from, "claude-opus-4-7");
            assert_eq!(to, "claude-opus-5");
        }
        other => panic!("expected declined, got {other:?}"),
    }
    let still = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    assert_eq!(still.model, original);
    assert!(still.fallback.is_none());
}

#[test]
fn refusal_and_truncation_retry_with_attempt_counter_then_ask() {
    let mut d = dispatcher();
    let task = TaskId::new("t");
    let dispatch = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    // retry_attempts = 2: two retries on the SAME model, then ask.
    match d.observe_turn(&task, &dispatch.model, refused()) {
        FallbackDecision::Retry { attempt } => assert_eq!(attempt, 1),
        other => panic!("expected retry, got {other:?}"),
    }
    match d.observe_turn(&task, &dispatch.model, refused()) {
        FallbackDecision::Retry { attempt } => assert_eq!(attempt, 2),
        other => panic!("expected retry, got {other:?}"),
    }
    match d.observe_turn(&task, &dispatch.model, refused()) {
        FallbackDecision::AskFallback(fr) => {
            assert_eq!(fr.from, dispatch.model);
            assert!(fr.reason.contains("refusal"));
        }
        other => panic!("expected ask, got {other:?}"),
    }
    // And max-tokens truncation follows the same modeled discipline.
    let t2 = TaskId::new("t2");
    let dispatch2 = d
        .dispatch_turn(&t2, RouteRequest::Role(Role::Editor))
        .expect("dispatch");
    match d.observe_turn(
        &t2,
        &dispatch2.model,
        TurnOutcome::Completed {
            stop: StopReason::MaxTokens,
        },
    ) {
        FallbackDecision::Retry { attempt } => assert_eq!(attempt, 1),
        other => panic!("expected retry, got {other:?}"),
    }
    // End turns NEVER propose a fallback.
    let t3 = TaskId::new("t3");
    let dispatch3 = d
        .dispatch_turn(&t3, RouteRequest::Role(Role::Editor))
        .expect("dispatch");
    assert_eq!(
        d.observe_turn(
            &t3,
            &dispatch3.model,
            TurnOutcome::Completed {
                stop: StopReason::EndTurn,
            }
        ),
        FallbackDecision::Proceed
    );
}

#[test]
fn rate_limits_wait_out_bounded_windows_before_failing_over() {
    use std::time::Duration;
    let mut d = dispatcher();
    let task = TaskId::new("t");
    let dispatch = d
        .dispatch_turn(&task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    // No hint uses the configured window; a shorter hint is honored.
    match d.observe_turn(
        &task,
        &dispatch.model,
        TurnOutcome::Failed {
            failure: DispatchFailure::RateLimit { retry_after: None },
        },
    ) {
        FallbackDecision::WaitOut { window, .. } => assert_eq!(window, Duration::from_secs(45)),
        other => panic!("expected wait-out, got {other:?}"),
    }
    match d.observe_turn(
        &task,
        &dispatch.model,
        TurnOutcome::Failed {
            failure: DispatchFailure::RateLimit {
                retry_after: Some(Duration::from_secs(10)),
            },
        },
    ) {
        FallbackDecision::WaitOut { window, .. } => assert_eq!(window, Duration::from_secs(10)),
        other => panic!("expected wait-out, got {other:?}"),
    }
    // Wait-outs beyond the retry budget escalate to an acknowledged ask.
    match d.observe_turn(
        &task,
        &dispatch.model,
        TurnOutcome::Failed {
            failure: DispatchFailure::RateLimit { retry_after: None },
        },
    ) {
        FallbackDecision::AskFallback(fr) => assert_eq!(fr.from, dispatch.model),
        other => panic!("expected ask, got {other:?}"),
    }
    // A 600s server hint on a FRESH task clamps to the configured window
    // (never beyond the 75s cap).
    let clamped_task = TaskId::new("clamped");
    let clamped = d
        .dispatch_turn(&clamped_task, RouteRequest::Role(Role::Architect))
        .expect("dispatch");
    match d.observe_turn(
        &clamped_task,
        &clamped.model,
        TurnOutcome::Failed {
            failure: DispatchFailure::RateLimit {
                retry_after: Some(Duration::from_secs(600)),
            },
        },
    ) {
        FallbackDecision::WaitOut { window, .. } => {
            assert_eq!(window, Duration::from_secs(45));
            assert!(window <= Duration::from_secs(75));
        }
        other => panic!("expected wait-out, got {other:?}"),
    }
}

#[test]
fn fallback_effort_falls_to_documented_default_when_off_grid() {
    // Given: commit-message pinned at effort none on gpt-5-5.
    let mut d = dispatcher();
    let task = TaskId::new("t");
    let dispatch = d
        .dispatch_turn(&task, RouteRequest::Role(Role::CommitMessage))
        .expect("dispatch");
    assert_eq!(dispatch.model.model, "gpt-5-5");
    assert_eq!(dispatch.effort, EffortValue::None);
    // When: outage -> gpt-5-5's chain hop sonnet is acknowledged. Sonnet's
    // grid has no `none` rung.
    let ask = match d.observe_turn(&task, &dispatch.model, outage()) {
        FallbackDecision::AskFallback(fr) => fr,
        other => panic!("expected ask, got {other:?}"),
    };
    assert_eq!(ask.to.model, "claude-sonnet-4-6");
    d.acknowledge_fallback(&task, ask.acknowledge())
        .expect("ack");
    let switched = d
        .dispatch_turn(&task, RouteRequest::Role(Role::CommitMessage))
        .expect("dispatch");
    // Then: the hop carries sonnet's DOCUMENTED default explicitly —
    // never the provider's implicit default, never an off-grid knob.
    assert_eq!(switched.model.model, "claude-sonnet-4-6");
    assert_eq!(switched.effort, EffortValue::Medium);
    assert_eq!(switched.knobs.effort, Some(EffortValue::Medium));
}
