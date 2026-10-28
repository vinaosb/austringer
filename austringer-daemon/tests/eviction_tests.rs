//! Idle-session eviction (wave 2b): an abandoned session host must
//! not pin its engine driver — and with it the `AgentLoop`'s
//! unbounded events mirror — for the daemon's lifetime. The sweep
//! closes the host's channels and drops the map entry; the engine
//! task observes the closed command lane and terminates; the next
//! touch re-creates the host with resume-safe semantics: NO history
//! re-publish (`published_through` resumes past replayed events) and
//! an intact cumulative cost ledger (the cap survives eviction).
//!
//! Hermetic: scripted inference, zero network, zero keys. Mirrors the
//! `executor_spill_tests` / `cost_ledger_tests` fixtures.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{InferenceError, InferenceRequest};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference, TurnUsage};
use austringer_core::event::{ContentBlock, Event};
use austringer_daemon::attach::{CommandRejected, HostCommand, SessionHost};
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::eviction;
use austringer_daemon::protocol::{ServerMsg, SessionEvent};
use austringer_daemon::server::DaemonState;
use austringer_daemon::usage::{ComposedInference, InferenceFactory};

use austringer_providers::identity::ModelIdentity;

/// Scripted inference over a SHARED step queue: consumption survives
/// host re-creation (each factory call yields a handle onto the same
/// deque), so a reopened session's next turn pops the NEXT step.
struct ScriptedInference {
    steps: Arc<Mutex<VecDeque<AssistantTurn>>>,
}

#[async_trait]
impl Inference for ScriptedInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let mut steps = match self.steps.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        steps
            .pop_front()
            .ok_or_else(|| InferenceError::Failure("script exhausted".into()))
    }
}

fn shared_script(steps: Vec<AssistantTurn>) -> (InferenceFactory, ()) {
    let queue = Arc::new(Mutex::new(VecDeque::from(steps)));
    let factory: InferenceFactory = {
        let queue = Arc::clone(&queue);
        Arc::new(move || {
            ComposedInference::plain(Box::new(ScriptedInference {
                steps: Arc::clone(&queue),
            }))
        })
    };
    (factory, ())
}

/// Inference whose first call blocks until a one-shot gate releases —
/// keeps an engine turn IN FLIGHT across a sweep.
struct GatedInference {
    gate: Arc<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
}

#[async_trait]
impl Inference for GatedInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let gate = match self.gate.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        if let Some(gate) = gate {
            let _ = gate.await;
        }
        Ok(plain_turn("gated done"))
    }
}

fn gated_factory() -> (InferenceFactory, tokio::sync::oneshot::Sender<()>) {
    let (release, gate) = tokio::sync::oneshot::channel();
    let cell = Arc::new(Mutex::new(Some(gate)));
    let factory: InferenceFactory = {
        let cell = Arc::clone(&cell);
        Arc::new(move || {
            ComposedInference::plain(Box::new(GatedInference {
                gate: Arc::clone(&cell),
            }))
        })
    };
    (factory, release)
}

/// Scripted PRICED inference (the claude-haiku-4-5 builtin row: input
/// $1/M tokens, so `input_tokens = N` books exactly N micro-USD per
/// call) — the cost_ledger_tests fixture, shared-queue flavored so
/// spend scripting survives host re-creation. Counts provider calls.
struct MeteredInference {
    per_call_costs: Arc<Mutex<VecDeque<u64>>>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl Inference for MeteredInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut costs = match self.per_call_costs.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let input = costs.pop_front().unwrap_or(0);
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text {
                text: "done".to_owned(),
            }],
            tool_calls: Vec::new(),
            usage: Some(TurnUsage {
                input_tokens: input,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            }),
        })
    }
}

fn metered_factory(costs: &[u64]) -> (InferenceFactory, Arc<std::sync::atomic::AtomicUsize>) {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let queue = Arc::new(Mutex::new(VecDeque::from(costs.to_vec())));
    let factory: InferenceFactory = {
        let queue = Arc::clone(&queue);
        let calls = Arc::clone(&calls);
        Arc::new(move || ComposedInference {
            inference: Box::new(MeteredInference {
                per_call_costs: Arc::clone(&queue),
                calls: Arc::clone(&calls),
            }),
            identity: Some(ModelIdentity::new("anthropic", "claude-haiku-4-5")),
        })
    };
    (factory, calls)
}

fn plain_turn(text: &str) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: Vec::new(),
        usage: None,
    }
}

fn daemon_at(dir: &Path, factory: InferenceFactory) -> Arc<DaemonState> {
    let config = DaemonConfig {
        data_dir: dir.to_owned(),
        token: Some("evict-token".into()),
        ..DaemonConfig::default()
    };
    Arc::new(DaemonState::new(config, factory).expect("daemon state"))
}

/// Blocks until the session's next `TurnFinished`, returning the
/// outcome plus every event frame that landed before it.
async fn turn_batch(live: &mut broadcast::Receiver<ServerMsg>) -> (String, Vec<SessionEvent>) {
    tokio::time::timeout(Duration::from_secs(30), async {
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
                Err(error) => panic!("hub died: {error:?}"),
            }
        }
    })
    .await
    .expect("turn never finished")
}

/// Poll the host's engine-task-finished observable (a deadline panic
/// names the failure: the engine task never terminated).
async fn wait_engine_finished(host: &Arc<SessionHost>) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !host.engine_task_finished() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("engine task never terminated after eviction");
}

/// Every text fragment a recorded event carries.
fn event_texts(event: &Event) -> Vec<&str> {
    let blocks = match event {
        Event::User { blocks } | Event::Assistant { blocks } => blocks,
        _ => return Vec::new(),
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// (1) the memory defect: an idle host is swept, its engine task ends
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eviction_sweep_releases_idle_host_and_ends_engine_task() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, _) = shared_script(vec![plain_turn("first")]);
    let state = daemon_at(dir.path(), factory);
    let sid = "s-evict";

    let host = state.session(sid);
    assert!(state.try_session(sid).is_some(), "host registered");

    let attach = host.attach("tui", None).expect("attach");
    let reader = tokio::spawn(async move {
        let mut live = attach.events.live;
        let mut recorded = 0usize;
        loop {
            match live.recv().await {
                Ok(ServerMsg::Event { frame }) => {
                    if matches!(frame.event, SessionEvent::Recorded { .. }) {
                        recorded += 1;
                    }
                    if matches!(frame.event, SessionEvent::TurnFinished { .. }) {
                        return recorded;
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(error) => panic!("hub died: {error:?}"),
            }
        }
    });
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    let recorded = tokio::time::timeout(Duration::from_secs(30), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    assert!(recorded > 0, "the turn produced recorded events");
    host.detach("tui");

    tokio::time::sleep(Duration::from_millis(10)).await;
    let evicted = eviction::run_once(&state, Duration::from_millis(1));
    assert_eq!(evicted, vec![sid.to_owned()], "the idle host is swept");
    assert!(state.try_session(sid).is_none(), "the map entry is gone");
    wait_engine_finished(&host).await;
}

// ---------------------------------------------------------------------------
// (2) never evict a host with attached clients (detach re-enables)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eviction_spares_attached_clients_until_detach() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, _) = shared_script(vec![plain_turn("first")]);
    let state = daemon_at(dir.path(), factory);
    let sid = "s-clients";

    let host = state.session(sid);
    host.attach("stay", None).expect("attach");
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        !host.is_idle(Duration::from_millis(1)),
        "an attached client keeps the host non-idle"
    );
    assert!(
        eviction::run_once(&state, Duration::from_millis(1)).is_empty(),
        "attached hosts survive the sweep"
    );
    assert!(state.try_session(sid).is_some());

    host.detach("stay");
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(
        eviction::run_once(&state, Duration::from_millis(1)),
        vec![sid.to_owned()],
        "detach re-enables eviction"
    );
}

// ---------------------------------------------------------------------------
// (3) never evict a host mid-turn (an active turn must complete)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eviction_spares_in_flight_turns_until_completion() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, release) = gated_factory();
    let state = daemon_at(dir.path(), factory);
    let sid = "s-busy";

    let host = state.session(sid);
    let mut live = host.hub_subscribe();
    host.handle_command(HostCommand::Submit("slow".into()))
        .await
        .expect("submit");

    // Wait until the turn is demonstrably in flight.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(ServerMsg::Event { frame }) = live.recv().await
                && matches!(frame.event, SessionEvent::TurnStarted { .. })
            {
                return;
            }
        }
    })
    .await
    .expect("turn never started");

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        eviction::run_once(&state, Duration::from_millis(1)).is_empty(),
        "an in-flight turn spares the host"
    );
    assert!(state.try_session(sid).is_some());

    release.send(()).expect("release the gated turn");
    let (outcome, _) = turn_batch(&mut live).await;
    assert!(
        !outcome.starts_with("failed"),
        "the active turn completes: {outcome}"
    );

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(
        eviction::run_once(&state, Duration::from_millis(1)),
        vec![sid.to_owned()],
        "idle again after the turn completes"
    );
    wait_engine_finished(&host).await;
}

// ---------------------------------------------------------------------------
// (4) resume-safety: reopen must NOT re-publish replayed history
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evicted_session_reopens_without_republishing_history() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, _) = shared_script(vec![
        plain_turn("first answer"),
        plain_turn("second answer"),
    ]);
    let state = daemon_at(dir.path(), factory);
    let sid = "s-resume";

    let host = state.session(sid);
    let mut live = host.hub_subscribe();
    host.handle_command(HostCommand::Submit("one".into()))
        .await
        .expect("submit one");
    let (_, first_batch) = turn_batch(&mut live).await;
    let first_max_seq = first_batch
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Recorded { seq, .. } => Some(*seq),
            _ => None,
        })
        .max()
        .expect("turn one recorded events");
    assert!(
        first_batch
            .iter()
            .flat_map(|event| match event {
                SessionEvent::Recorded { event, .. } => event_texts(event),
                _ => Vec::new(),
            })
            .any(|text| text.contains("first answer")),
        "turn one's answer is in the first batch"
    );

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(
        eviction::run_once(&state, Duration::from_millis(1)),
        vec![sid.to_owned()]
    );
    wait_engine_finished(&host).await;

    // Re-open: a NEW host over the SAME session dir (AgentLoop::open
    // replays turn one's events into its mirror).
    let reopened = state.session(sid);
    assert!(
        !Arc::ptr_eq(&host, &reopened),
        "a fresh host was created for the evicted id"
    );
    let mut live2 = reopened.hub_subscribe();
    reopened
        .handle_command(HostCommand::Submit("two".into()))
        .await
        .expect("submit two");
    let (_, second_batch) = turn_batch(&mut live2).await;

    let second_recorded: Vec<(u64, &Event)> = second_batch
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Recorded { seq, event } => Some((*seq, event)),
            _ => None,
        })
        .collect();
    assert!(
        !second_recorded.is_empty(),
        "turn two recorded events: {second_batch:?}"
    );
    for (seq, _) in &second_recorded {
        assert!(
            *seq > first_max_seq,
            "no historical frame re-published: seq {seq} <= {first_max_seq}"
        );
    }
    for (_, event) in &second_recorded {
        for text in event_texts(event) {
            assert!(
                !text.contains("first answer"),
                "replayed history must stay unpublished: {text:?}"
            );
        }
    }
    assert!(
        second_recorded.iter().any(|(_, event)| event_texts(event)
            .iter()
            .any(|t| t.contains("second answer"))),
        "turn two's own answer published"
    );
}

// ---------------------------------------------------------------------------
// (5) resume-safety: cumulative spend survives eviction, cap enforced
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evicted_session_cost_ledger_and_cap_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let (factory, calls) = metered_factory(&[500_000, 600_000]);
    let config = DaemonConfig {
        data_dir: dir.path().to_owned(),
        token: Some("evict-token".into()),
        durable: false,
        session_cost_cap_micros: 1_000_000,
        ..DaemonConfig::default()
    };
    let state = Arc::new(DaemonState::new(config, factory).expect("daemon state"));
    let sid = "s-cost-evict";

    let host = state.session(sid);
    let mut live = host.hub_subscribe();
    for prompt in ["one", "two"] {
        host.handle_command(HostCommand::Submit(prompt.into()))
            .await
            .expect("submit accepted");
        let (outcome, _) = turn_batch(&mut live).await;
        assert!(!outcome.starts_with("failed"), "{prompt}: {outcome}");
    }
    let spent = host.cost_ledger().expect("ledger bound").snapshot();
    assert_eq!(spent.consumed_micros, 1_100_000);
    assert!(spent.stopped, "the cap crossed before eviction");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(
        eviction::run_once(&state, Duration::from_millis(1)),
        vec![sid.to_owned()]
    );
    wait_engine_finished(&host).await;

    let reopened = state.session(sid);
    let ledger = reopened.cost_ledger().expect("ledger bound on reopen");
    let resumed = ledger.snapshot();
    assert_eq!(
        resumed.consumed_micros, 1_100_000,
        "cumulative spend survives eviction"
    );
    assert_eq!(resumed.cap_micros, 1_000_000);
    assert!(resumed.stopped, "the stop flag survives eviction");

    // The cost-cap contract still enforces across the eviction: new
    // turns are refused typed, and no provider call happens.
    let rejection = reopened
        .handle_command(HostCommand::Submit("three".into()))
        .await
        .expect_err("at-cap submit refused after reopen");
    assert!(
        matches!(rejection, CommandRejected::CostCapReached(_)),
        "typed cost refusal, got {rejection:?}"
    );
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "no provider call after reopen"
    );
}

// ---------------------------------------------------------------------------
// (6) config: 1h default, ZERO disables
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eviction_config_default_is_one_hour_and_zero_disables() {
    assert_eq!(
        DaemonConfig::default().session_idle_eviction,
        Duration::from_secs(3600),
        "the default idle threshold is one hour (the janitor cadence)"
    );

    // ZERO is the disabled posture: even an ancient idle host stays.
    let dir = tempfile::tempdir().unwrap();
    let (factory, _) = shared_script(Vec::new());
    let state = daemon_at(dir.path(), factory);
    let host = state.session("s-off");
    host.attach("tui", None).unwrap();
    host.detach("tui");
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        eviction::run_once(&state, Duration::ZERO).is_empty(),
        "ZERO threshold disables the sweep"
    );
    assert!(state.try_session("s-off").is_some());
}
