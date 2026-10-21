//! Task-33 acceptance (plan todo 33): the money ledger wired into the
//! daemon engine loop. The EXISTING harness-core budget vocabulary
//! (`SpendLedger`, `MicroUsd`, 80/100 thresholds) governs every session:
//! priced spend from Usage accounting books into a per-session ledger;
//! 80% emits a typed `CostWarning` (+ badge state), 100% a typed
//! `CostStop` that halts further turns with NO further provider calls;
//! launch children aggregate into the parent ledger; scheduled sessions
//! own their caps; a pending fallback ask resolves (same-model retry)
//! BEFORE the stop lands — never a silent substitution.
//!
//! Hermetic: scripted priced inference (claude-haiku-4-5 rows — under
//! the builtin $1/M input price, one input token books exactly one
//! micro-USD), zero network, zero keys.

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use harness_core::agent_loop::inference::InferenceRequest;
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference, TurnUsage};
use harness_core::event::ContentBlock;
use harness_daemon::attach::{CommandRejected, HostCommand, SessionHost};
use harness_daemon::config::{DaemonConfig, parse_session_cost_cap_usd};
use harness_daemon::cost_ledger::DEFAULT_SESSION_COST_CAP_MICROS;
use harness_daemon::protocol::{ServerMsg, SessionEvent};
use harness_daemon::server::DaemonState;
use harness_daemon::usage::ComposedInference;

use harness_providers::identity::{FallbackRequest, ModelIdentity};

/// The priced fixture identity: builtin row input $1/M tokens, so
/// `input_tokens = N` books exactly `N` micro-USD per call.
fn haiku() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-haiku-4-5")
}

/// Two parallel workers with disjoint write sets (the happy launch
/// fixture from the todo-8 suite).
const DISJOINT_SPEC: &str = "\
objective: prove the aggregated cost ledger
task t1:
  role: worker
  objective: map the layout
  writes: src/**
task t2:
  role: worker
  objective: write the docs
  writes: docs/**
";

/// Scripted completion-only inference metered per call: call N returns
/// `per_call_costs[N]` input tokens (== that many micro-USD under the
/// haiku row) and completes the task. Counts every provider call.
struct MeteredInference {
    per_call_costs: Vec<u64>,
    next: usize,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Inference for MeteredInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, harness_core::agent_loop::InferenceError> {
        let index = self.next;
        self.next += 1;
        self.calls.fetch_add(1, Ordering::SeqCst);
        let input = self.per_call_costs.get(index).copied().unwrap_or(0);
        Ok(metered_turn(input))
    }
}

/// The pending-ask fixture: the SECOND call mediates a fallback ask
/// (deny-side resolution = same-model retry) BEFORE returning the spend
/// that crosses the cap — the ask-first path inside one inference call.
struct FallbackMeteredInference {
    host_cell: Arc<OnceLock<Arc<SessionHost>>>,
    per_call_costs: Vec<u64>,
    next: usize,
}

#[async_trait]
impl Inference for FallbackMeteredInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, harness_core::agent_loop::InferenceError> {
        let index = self.next;
        self.next += 1;
        if index == 1
            && let Some(host) = self.host_cell.get()
        {
            // Routing proposes a hop; the user disposes. The deny-side
            // answer (chosen == current) retries the SAME model.
            let _ = host
                .mediate_fallback(FallbackRequest::new(
                    haiku(),
                    ModelIdentity::new("anthropic", "claude-sonnet-4-5"),
                    "fixture outage",
                ))
                .await;
        }
        Ok(metered_turn(self.per_call_costs.get(index).copied().unwrap_or(0)))
    }
}

fn metered_turn(input_tokens: u64) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: "done".to_owned(),
        }],
        // No tool calls: a plain turn ends TurnComplete and the session
        // stays live for the next submit (task_completed would make the
        // session terminal and mask the cost-cap behaviors under
        // `failed::Terminal`).
        tool_calls: Vec::new(),
        usage: Some(TurnUsage {
            input_tokens,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        }),
    }
}

/// The daemon fixture: production `DaemonState` session factory over a
/// priced scripted stack, with the session cost cap forced small.
fn daemon_with(
    dir: &std::path::Path,
    factory: harness_daemon::usage::InferenceFactory,
    cap_micros: u128,
) -> Arc<DaemonState> {
    let config = DaemonConfig {
        data_dir: dir.to_owned(),
        token: Some("cost-token".into()),
        durable: false,
        session_cost_cap_micros: cap_micros,
        ..DaemonConfig::default()
    };
    Arc::new(DaemonState::new(config, factory).unwrap_or_else(|e| panic!("state: {e}")))
}

/// A priced scripted factory plus its shared provider-call counter.
fn metered_factory(costs: &[u64]) -> (harness_daemon::usage::InferenceFactory, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let costs = costs.to_vec();
    let factory = {
        let calls = Arc::clone(&calls);
        Arc::new(move || ComposedInference {
            inference: Box::new(MeteredInference {
                per_call_costs: costs.clone(),
                next: 0,
                calls: Arc::clone(&calls),
            }),
            identity: Some(haiku()),
        })
    };
    (factory, calls)
}

async fn submit(host: &Arc<SessionHost>, text: &str) -> Result<(), CommandRejected> {
    host.handle_command(HostCommand::Submit(text.to_owned())).await
}

/// Blocks until the session's next `TurnFinished`, returning the
/// outcome summary plus every event frame that landed before it (the
/// broadcast receiver is single-consumption — a later `drain` only
/// sees what follows).
async fn turn_batch(live: &mut broadcast::Receiver<ServerMsg>) -> (String, Vec<SessionEvent>) {
    let deadline = Duration::from_secs(30);
    tokio::time::timeout(deadline, async {
        let mut seen = Vec::new();
        loop {
            match live.recv().await {
                Ok(ServerMsg::Event { frame }) => {
                    if let SessionEvent::TurnFinished { outcome, .. } = &frame.event {
                        return (outcome.clone(), seen);
                    }
                    seen.push(frame.event);
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(e) => panic!("hub died: {e:?}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("turn never finished"))
}

fn drain(live: &mut broadcast::Receiver<ServerMsg>) -> Vec<SessionEvent> {
    let mut out = Vec::new();
    while let Ok(msg) = live.try_recv() {
        if let ServerMsg::Event { frame } = msg {
            out.push(frame.event);
        }
    }
    out
}

fn cost_warnings(events: &[SessionEvent]) -> Vec<(u128, u128)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::CostWarning { consumed_micros, cap_micros } => {
                Some((*consumed_micros, *cap_micros))
            }
            _ => None,
        })
        .collect()
}

fn cost_stops(events: &[SessionEvent]) -> Vec<(u128, u128)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::CostStop { consumed_micros, cap_micros } => {
                Some((*consumed_micros, *cap_micros))
            }
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// (1) 80% crossing: typed Warning event + observable badge state
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cost_ledger_warning_at_80_pct_emits_typed_event_and_badge_state() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, calls) = metered_factory(&[500_000, 350_000]);
    let state = daemon_with(dir.path(), factory, 1_000_000);
    let host = state.session("s-warn");
    let mut live = host.hub_subscribe();

    submit(&host, "one").await.expect("under-cap submit accepted");
    let (first, mut events) = turn_batch(&mut live).await;
    assert!(!first.starts_with("failed"), "turn one must succeed: {first}");

    submit(&host, "two").await.expect("still-under-cap submit accepted");
    let (second, mut batch) = turn_batch(&mut live).await;
    assert!(!second.starts_with("failed"), "turn two must succeed: {second}");
    events.append(&mut batch);
    events.extend(drain(&mut live));

    let warnings = cost_warnings(&events);
    assert_eq!(warnings.len(), 1, "exactly one warning: {events:?}");
    assert_eq!(warnings[0], (850_000, 1_000_000), "850k of 1M is 85%");
    assert!(cost_stops(&events).is_empty(), "no stop under the cap");

    // HUD badge state (todos 19/20 consume this seam).
    let badge = host.cost_ledger().expect("ledger bound").snapshot();
    assert_eq!(badge.consumed_micros, 850_000);
    assert_eq!(badge.cap_micros, 1_000_000);
    assert!(badge.warned);
    assert!(!badge.stopped);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

// ---------------------------------------------------------------------------
// (2) 100% crossing: typed Stop, further turns halted, NO further
//     provider calls, submits refused typed naming the cap
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cost_ledger_warning_stop_at_cap_halts_turns_typed() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, calls) = metered_factory(&[500_000, 600_000]);
    let state = daemon_with(dir.path(), factory, 1_000_000);
    let host = state.session("s-stop");
    let mut live = host.hub_subscribe();

    submit(&host, "one").await.expect("accepted");
    let (first, mut events) = turn_batch(&mut live).await;
    assert!(!first.starts_with("failed"), "turn one must succeed: {first}");
    submit(&host, "two").await.expect("accepted");
    let (second, mut batch) = turn_batch(&mut live).await;
    assert!(!second.starts_with("failed"), "turn two must succeed: {second}");
    events.append(&mut batch);
    events.extend(drain(&mut live));

    let warnings = cost_warnings(&events);
    assert_eq!(warnings.len(), 1, "the crossing record warns: {events:?}");
    let stops = cost_stops(&events);
    assert_eq!(stops.len(), 1, "exactly one typed stop: {events:?}");
    assert_eq!(stops[0], (1_100_000, 1_000_000), "overshoot is recorded");

    // Further Submit attempts: a TYPED refusal naming the cap — never a
    // silent drop, never a model switch.
    let rejection = submit(&host, "three").await.expect_err("at-cap submit refused");
    let CommandRejected::CostCapReached(reason) = &rejection else {
        panic!("typed cost refusal expected, got {rejection:?}");
    };
    let text = reason.to_string();
    assert!(text.contains("1000000"), "refusal names the cap: {text}");
    assert!(text.contains("session"), "refusal names the period: {text}");

    // A submit that was already queued (the scheduler/kicker path
    // bypasses intake) still makes NO further provider call: the
    // ledger's preflight gate refuses inference before the provider
    // seam. The loop reports turn_complete for such turns (its
    // StopHook consult order swallows first-inference failures on
    // multi-turn sessions — a pre-existing loop quirk, out of scope
    // here); the provider-call counter is the halt proof.
    host.commands()
        .send(HostCommand::Submit("queued".into()))
        .await
        .expect("queue accepts");
    let (third, mut batch) = turn_batch(&mut live).await;
    events.append(&mut batch);
    events.extend(drain(&mut live));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "no provider call past the cap (refused turn outcome: {third})"
    );
    // No assistant turn was produced for the refused turn: exactly the
    // two metered turns recorded assistant events.
    let assistant_turns = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                SessionEvent::Recorded { event: harness_core::event::Event::Assistant { .. }, .. }
            )
        })
        .count();

    let badge = host.cost_ledger().expect("ledger bound").snapshot();
    assert!(badge.stopped);
    assert_eq!(badge.consumed_micros, 1_100_000);
    assert_eq!(assistant_turns, 2, "no assistant turn for the refused turn: {events:?}");
}

// ---------------------------------------------------------------------------
// (3) launch children: spend aggregates into the PARENT session ledger
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cost_ledger_child_spend_aggregates_into_parent_ledger() {
    let dir = tempfile::tempdir().unwrap();
    // Each child's single turn books 1.2M micro-USD; two children
    // aggregate to 2.4M against the parent's 2M cap (warn at 1.6M).
    let (factory, calls) = metered_factory(&[1_200_000]);
    let state = daemon_with(dir.path(), factory, 2_000_000);
    let host = state.session("s-team-cost");
    let mut live = host.hub_subscribe();

    let spec_path = dir.path().join("cost-spec.md");
    std::fs::write(&spec_path, DISJOINT_SPEC).expect("spec fixture");
    submit(&host, &format!("/launch {}", spec_path.display()))
        .await
        .expect("launch accepted");

    let events = {
        let deadline = Duration::from_secs(40);
        tokio::time::timeout(deadline, async {
            let mut seen = Vec::new();
            loop {
                match live.recv().await {
                    Ok(ServerMsg::Event { frame }) => {
                        let finished = matches!(
                            frame.event,
                            SessionEvent::ChildFinished { .. }
                        );
                        seen.push(frame.event);
                        let finished_count = seen
                            .iter()
                            .filter(|e| matches!(e, SessionEvent::ChildFinished { .. }))
                            .count();
                        if finished && finished_count == 2 {
                            return seen;
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(e) => panic!("hub died: {e:?}"),
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("launch never finished"))
    };

    for child in ["cost-spec-child-t1", "cost-spec-child-t2"] {
        assert!(
            events.iter().any(|event| matches!(
                event,
                SessionEvent::ChildStarted { child: c, .. } if c == child
            )),
            "{child} started: {events:?}"
        );
    }

    // The PARENT ledger carries both children's spend; the typed
    // warning AND stop fired on the parent session's stream.
    let badge = host.cost_ledger().expect("parent ledger bound").snapshot();
    assert_eq!(badge.consumed_micros, 2_400_000, "both children aggregated");
    assert_eq!(badge.cap_micros, 2_000_000);
    assert!(badge.warned, "2.4M crossed the 1.6M warn line");
    assert!(badge.stopped, "2.4M crossed the 2M stop line");
    assert_eq!(cost_warnings(&events).len(), 1, "{events:?}");
    assert_eq!(cost_stops(&events).len(), 1, "{events:?}");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "one provider call per child");
}

// ---------------------------------------------------------------------------
// (4) automation sessions own their caps (the scheduled construction
//     path; kicker submits bypass intake — the preflight gate governs)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cost_ledger_automation_session_respects_own_cap() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, calls) = metered_factory(&[600_000, 600_000]);
    let state = daemon_with(dir.path(), factory, 1_000_000);
    let host = state.session_scheduled("auto-costcap-1", Some(5));
    let mut live = host.hub_subscribe();

    let mut events = Vec::new();
    for run in 0..3 {
        host.commands()
            .send(HostCommand::Submit("scheduled run".into()))
            .await
            .expect("queue accepts");
        let (outcome, mut batch) = turn_batch(&mut live).await;
        events.append(&mut batch);
        assert!(
            !outcome.starts_with("failed"),
            "runs never fail loudly (the cap halts at the provider seam): run {run}: {outcome}"
        );
    }
    events.extend(drain(&mut live));

    // Two metered assistant turns; the third fire made NO provider
    // call (the preflight gate refused it — kicker submits bypass the
    // intake refusal, so the provider seam is the automation cap).
    assert_eq!(calls.load(Ordering::SeqCst), 2, "no provider call past the cap");
    let assistant_turns = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                SessionEvent::Recorded { event: harness_core::event::Event::Assistant { .. }, .. }
            )
        })
        .count();
    assert_eq!(assistant_turns, 2, "exactly two metered runs: {events:?}");
    assert_eq!(cost_warnings(&events).len(), 1, "{events:?}");
    assert_eq!(cost_stops(&events).len(), 1, "{events:?}");
    assert!(host.cost_ledger().expect("ledger bound").snapshot().stopped);
}

// ---------------------------------------------------------------------------
// (5) FAILURE scenario (plan QA): a session at cap with a PENDING
//     fallback ask resolves the ask (same-model retry) BEFORE the Stop
//     lands — no silent substitution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cost_ledger_pending_fallback_ask_resolves_before_stop_no_substitution() {
    let dir = tempfile::tempdir().unwrap();
    let host_cell = Arc::new(OnceLock::new());
    let factory = {
        let host_cell = Arc::clone(&host_cell);
        Arc::new(move || ComposedInference {
            // The cell resolves at infer time (call 2), long after the
            // session factory has bound the host.
            inference: Box::new(FallbackMeteredInference {
                host_cell: Arc::clone(&host_cell),
                per_call_costs: vec![500_000, 600_000],
                next: 0,
            }),
            identity: Some(haiku()),
        })
    };
    let state = daemon_with(dir.path(), factory, 1_000_000);
    let host = state.session("s-ask");
    assert!(host_cell.set(Arc::clone(&host)).is_ok(), "bind host cell");
    let mut live = host.hub_subscribe();

    submit(&host, "one").await.expect("accepted");
    let (first, mut events) = turn_batch(&mut live).await;
    assert!(!first.starts_with("failed"), "turn one must succeed: {first}");

    // Answer the in-flight fallback ask DENY-side (chosen == current):
    // the mediation resolves same-model — the ask is never swallowed.
    let answerer = tokio::spawn({
        let host = Arc::clone(&host);
        async move {
            let mut control = host.mediator().subscribe();
            loop {
                match control.recv().await {
                    Ok(ServerMsg::FallbackAsk { ask }) => {
                        let _ = host
                            .mediator()
                            .answer_fallback(ask.ask_id, "test-client", ask.current.clone());
                        return;
                    }
                    Ok(_) => continue,
                    Err(e) => panic!("control bus died: {e:?}"),
                }
            }
        }
    });

    submit(&host, "two").await.expect("accepted");
    let (second, mut batch) = turn_batch(&mut live).await;
    assert!(!second.starts_with("failed"), "the mediated turn succeeds: {second}");
    events.append(&mut batch);
    events.extend(drain(&mut live));
    answerer.await.expect("ask answered");

    // ORDER: the fallback RESOLUTION lands before the typed Stop, and
    // the resolution targets the SAME model (deny-side retry).
    let stop_index = events
        .iter()
        .position(|event| matches!(event, SessionEvent::CostStop { .. }))
        .expect("typed stop landed");
    let resolved_index = events
        .iter()
        .position(
            |event| matches!(
                event,
                SessionEvent::Notification { kind, message }
                    if kind == "fallback" && message.contains("resolved")
            ),
        )
        .expect("fallback resolution landed");
    assert!(
        resolved_index < stop_index,
        "ask resolves BEFORE the stop: resolved@{resolved_index} stop@{stop_index}: {events:?}"
    );
    let SessionEvent::Notification { message, .. } = &events[resolved_index] else {
        unreachable!()
    };
    assert!(
        message.contains("claude-haiku-4-5"),
        "same-model retry, no substitution: {message}"
    );
    assert_eq!(cost_stops(&events).len(), 1);

    // After the stop: typed refusal, no model ever switched.
    let rejection = submit(&host, "three").await.expect_err("at-cap submit refused");
    assert!(matches!(rejection, CommandRejected::CostCapReached(..)));
}

// ---------------------------------------------------------------------------
// (6) unpriced models: honest absence — a cap never trips on unknowns
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cost_ledger_unpriced_turns_do_not_trip_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let factory: harness_daemon::usage::InferenceFactory = Arc::new(|| {
        ComposedInference {
            inference: Box::new(MeteredInference {
                per_call_costs: vec![900_000_000, 900_000_000],
                next: 0,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            identity: Some(ModelIdentity::new("fake", "unknown-model")),
        }
    });
    let state = daemon_with(dir.path(), factory, 1_000_000);
    let host = state.session("s-unpriced");
    let mut live = host.hub_subscribe();

    let mut events = Vec::new();
    for _ in 0..2 {
        submit(&host, "burn").await.expect("unpriced submits never refused");
        let (outcome, mut batch) = turn_batch(&mut live).await;
        assert!(!outcome.starts_with("failed"), "unpriced turns run: {outcome}");
        events.append(&mut batch);
    }
    events.extend(drain(&mut live));
    assert!(
        cost_warnings(&events).is_empty() && cost_stops(&events).is_empty(),
        "a cap cannot trip on unknown costs: {events:?}"
    );
    let badge = host.cost_ledger().expect("ledger bound").snapshot();
    assert_eq!(badge.consumed_micros, 0, "unpriced spend books zero money");
    assert!(!badge.warned && !badge.stopped);
    let unpriced_frames = events
        .iter()
        .filter(|event| matches!(event, SessionEvent::Recorded { .. }))
        .count();
    assert!(unpriced_frames > 0, "engine events still flow: {events:?}");
}

// ---------------------------------------------------------------------------
// (7) config: the default cap + the user-global decimal-dollar override
// ---------------------------------------------------------------------------

#[test]
fn cost_ledger_config_default_is_twenty_dollars_per_session() {
    assert_eq!(DaemonConfig::default().session_cost_cap_micros, DEFAULT_SESSION_COST_CAP_MICROS);
    assert_eq!(DEFAULT_SESSION_COST_CAP_MICROS, 20_000_000, "rationale in the constant doc");
}

#[test]
fn cost_ledger_config_override_parses_decimal_dollars_integer_only() {
    assert_eq!(parse_session_cost_cap_usd("5"), Ok(5_000_000));
    assert_eq!(parse_session_cost_cap_usd("12.50"), Ok(12_500_000));
    assert_eq!(parse_session_cost_cap_usd("0.000001"), Ok(1));
    for bad in ["", "-1", "1.2345678", "abc", "1.2.3"] {
        assert!(
            parse_session_cost_cap_usd(bad).is_err(),
            "{bad:?} must be rejected typed"
        );
    }
}
