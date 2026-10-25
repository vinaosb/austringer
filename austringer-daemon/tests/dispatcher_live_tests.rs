//! Dispatcher-live acceptance (plan todo 9, ecosystem alignment): session
//! turns route through austringer-intel's `Dispatcher` from the daemon engine
//! — role routes from routing.json, stickiness pins across resume, and
//! ask-first fallbacks mediated over todo 1's `FallbackAsk` machinery.
//! The identity contract is the FAILURE scenario's whole point: an
//! unacknowledged (denied / cancelled / timed-out) fallback ask retries
//! the SAME model — zero substitution events, ever.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{InferenceError, InferenceRequest, TurnUsage};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::ContentBlock;

use austringer_daemon::attach::HostCommand;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::mediation::PermissionMediator;
use austringer_daemon::protocol::{FallbackAskPayload, ServerMsg, SessionEvent};
use austringer_daemon::routing::{ModelSource, RoutingIntake};
use austringer_daemon::server::DaemonState;
use austringer_daemon::usage::ComposedInference;
use austringer_intel::catalog::Catalog;
use austringer_intel::routing::RoutingTable;
use austringer_providers::identity::ModelIdentity;

/// routing.json with a `default` role on claude-opus-4-7 (catalog default
/// effort xhigh) and a one-hop chain onto gemini-3-pro — whose grid is
/// low/medium/high, so an acknowledged hop MUST revalidate the pinned
/// xhigh rung down to gemini's documented default (dispatch.rs:94-117).
const DISPATCH_ROUTING: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["claude-sonnet-4-6"],
  "roles": [
    { "role": "default", "model": "claude-opus-4-7" },
    { "role": "summarizer", "model": "claude-sonnet-4-6" }
  ],
  "fallbacks": [
    { "model": "claude-opus-4-7", "chain": ["gemini-3-pro"] }
  ],
  "rate_limit_wait_out_secs": 1,
  "retry_attempts": 1
}
"#;

/// routing.json with NO entry for the session role: every turn must fall
/// back to the session's CONFIGURED model explicitly (no implicit hop).
const UNROUTED: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["claude-sonnet-4-6"],
  "roles": [
    { "role": "summarizer", "model": "claude-sonnet-4-6" }
  ]
}
"#;

/// The Wave-R→Wave-U degraded-mode sentence (plan todo 9): verbatim.
const DEGRADED_RETRY_NOTICE: &str =
    "model fallback asked; no interactive client — retrying same model";

/// The session's configured identity for every test (distinct family +
/// model from the routing table so a mis-route is unambiguous).
fn configured_identity() -> ModelIdentity {
    ModelIdentity::new("xai", "grok-4-6")
}

/// One scripted step of a model's stack: a completed turn or a failure.
#[derive(Clone)]
enum Step {
    Turn(AssistantTurn),
    Fail(String),
}

fn usage_turn(text: &str) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: Vec::new(),
        usage: Some(TurnUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        }),
    }
}

fn outage_step() -> Step {
    Step::Fail("provider unreachable: connection refused".to_owned())
}

/// What the fake provider seam observed, per model: stack LOOKUPS (how
/// many times the dispatcher asked for a model) and INFER calls (how
/// many turns each stack actually served).
#[derive(Default)]
struct RouterLog {
    lookups: Vec<String>,
    infer_calls: Vec<String>,
}

struct FakeModels {
    log: Arc<Mutex<RouterLog>>,
    scripts: Mutex<HashMap<String, VecDeque<Step>>>,
}

impl FakeModels {
    fn new(log: Arc<Mutex<RouterLog>>, scripts: Vec<(&str, Vec<Step>)>) -> Arc<Self> {
        Arc::new(Self {
            log,
            scripts: Mutex::new(
                scripts
                    .into_iter()
                    .map(|(model, steps)| (model.to_owned(), VecDeque::from(steps)))
                    .collect(),
            ),
        })
    }

    /// The per-model provider-stack source the routing intake injects.
    fn source(self: &Arc<Self>) -> ModelSource {
        let fake = Arc::clone(self);
        Arc::new(move |model: &str| {
            {
                let mut log = fake.log.lock().expect("log");
                log.lookups.push(model.to_owned());
            }
            let mut scripts = fake.scripts.lock().expect("scripts");
            let steps = scripts.remove(model)?;
            Some(Box::new(ModelScript {
                model: model.to_owned(),
                steps: Mutex::new(steps),
                log: Arc::clone(&fake.log),
            }) as Box<dyn Inference>)
        })
    }
}

struct ModelScript {
    model: String,
    steps: Mutex<VecDeque<Step>>,
    log: Arc<Mutex<RouterLog>>,
}

#[async_trait]
impl Inference for ModelScript {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        {
            let mut log = self.log.lock().expect("log");
            log.infer_calls.push(self.model.clone());
        }
        let mut steps = self.steps.lock().expect("steps");
        match steps.pop_front() {
            Some(Step::Turn(turn)) => Ok(turn),
            Some(Step::Fail(message)) => Err(InferenceError::Failure(message)),
            None => Err(InferenceError::Failure("script exhausted".to_owned())),
        }
    }
}

/// A daemon state whose sessions dispatch through the given routing
/// document over the scripted per-model stacks; the factory carries the
/// CONFIGURED identity (the explicit un-routed route).
fn routed_state(
    routing_json: &str,
    ask_timeout: Duration,
    scripts: Vec<(&str, Vec<Step>)>,
) -> (Arc<DaemonState>, Arc<Mutex<RouterLog>>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = DaemonConfig {
        data_dir: dir.path().to_path_buf(),
        ask_timeout,
        ..DaemonConfig::default()
    };
    let log = Arc::new(Mutex::new(RouterLog::default()));
    let models = FakeModels::new(Arc::clone(&log), scripts);
    let table = RoutingTable::from_json(routing_json, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("routing doc rejected: {e:?}"));
    let factory: austringer_daemon::usage::InferenceFactory = {
        // The factory's own stack is REPLACED by the routing intake on
        // the routed path; if it ever served, the turn fails loudly
        // here (proving the dispatcher owns the turn path).
        Arc::new(|| ComposedInference {
            inference: Box::new(NeverServes),
            identity: Some(configured_identity()),
        })
    };
    let mut state = DaemonState::new(config, factory).expect("daemon state");
    state.routing = Some(Arc::new(RoutingIntake {
        table,
        models: models.source(),
    }));
    (Arc::new(state), log, dir)
}

struct NeverServes;

#[async_trait]
impl Inference for NeverServes {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Err(InferenceError::Failure(
            "factory stack must never serve on the routed path".to_owned(),
        ))
    }
}

/// Everything one submitted turn (or N turns) surfaces on the two lanes
/// a client sees: the hub (events + Usage broadcasts) and the control
/// bus (FallbackAsk / FallbackResolved).
struct TurnCapture {
    usage: Vec<(String, String, Option<String>)>,
    notifications: Vec<(String, String)>,
    fallback_asks: Vec<FallbackAskPayload>,
    fallback_resolved: Vec<ModelIdentity>,
    finished: usize,
}

impl TurnCapture {
    fn degraded_retry_notices(&self) -> usize {
        self.notifications
            .iter()
            .filter(|(_, message)| message == DEGRADED_RETRY_NOTICE)
            .count()
    }
}

/// Drains both channels until they are momentarily empty, recording the
/// frames `read_turns` cares about. Fixes a select! race: TurnFinished and
/// FallbackResolved are frequently ready simultaneously on the two
/// channels, and select! randomization could return the reader with the
/// pending control frame (e.g. FallbackResolved) still unread - coverage
/// instrumentation widens that coincidence window.
async fn drain_late_frames(
    events: &mut broadcast::Receiver<ServerMsg>,
    control: &mut broadcast::Receiver<ServerMsg>,
    capture: &mut TurnCapture,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let mut received = false;
        while let Ok(msg) = events.try_recv() {
            received = true;
            if let ServerMsg::Usage { model, effort, .. } = msg {
                capture.usage.push((model.provider, model.model, effort));
            }
        }
        while let Ok(msg) = control.try_recv() {
            received = true;
            match msg {
                ServerMsg::FallbackAsk { ask } => capture.fallback_asks.push(ask),
                ServerMsg::FallbackResolved { chosen, .. } => {
                    capture.fallback_resolved.push(chosen);
                }
                _ => {}
            }
        }
        if !received || tokio::time::Instant::now() >= deadline {
            return;
        }
    }
}

async fn read_turns(
    mut events: broadcast::Receiver<ServerMsg>,
    mut control: broadcast::Receiver<ServerMsg>,
    want_finished: usize,
) -> TurnCapture {
    let mut capture = TurnCapture {
        usage: Vec::new(),
        notifications: Vec::new(),
        fallback_asks: Vec::new(),
        fallback_resolved: Vec::new(),
        finished: 0,
    };
    loop {
        tokio::select! {
            msg = events.recv() => match msg {
                Ok(ServerMsg::Event { frame }) => match frame.event {
                    SessionEvent::TurnFinished { .. } => {
                        capture.finished += 1;
                        if capture.finished >= want_finished {
                            drain_late_frames(&mut events, &mut control, &mut capture).await;
                            return capture;
                        }
                    }
                    SessionEvent::Notification { kind, message } => {
                        capture.notifications.push((kind, message));
                    }
                    _ => {}
                },
                Ok(ServerMsg::Usage { model, effort, .. }) => {
                    capture.usage.push((model.provider, model.model, effort));
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return capture,
            },
            msg = control.recv() => match msg {
                Ok(ServerMsg::FallbackAsk { ask }) => capture.fallback_asks.push(ask),
                Ok(ServerMsg::FallbackResolved { chosen, .. }) => {
                    capture.fallback_resolved.push(chosen);
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => {}
            },
        }
    }
}

/// Submits `turns` prompts on the session and returns what a client
/// observed (bounded: a silent hang becomes a named failure).
async fn drive_turns(state: &DaemonState, session: &str, turns: usize) -> TurnCapture {
    let host = state.session(session);
    let attachment = host.attach("tui", None).expect("attach");
    let reader = tokio::spawn(read_turns(
        attachment.events.live,
        attachment.control,
        turns,
    ));
    for _ in 0..turns {
        let _ = host
            .handle_command(HostCommand::Submit("hello".into()))
            .await;
    }
    match tokio::time::timeout(Duration::from_secs(30), reader).await {
        Ok(joined) => joined.expect("reader task"),
        Err(_) => panic!("no TurnFinished x{turns} within 30s"),
    }
}

/// Subscribes (synchronously, BEFORE any submit) and answers the first
/// fallback ask with its own proposal — the acknowledged-hop client.
fn spawn_fallback_answerer(mediator: Arc<PermissionMediator>) -> tokio::task::JoinHandle<()> {
    let mut watch = mediator.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = watch.recv().await {
            if let ServerMsg::FallbackAsk { ask } = msg {
                mediator
                    .answer_fallback(ask.ask_id, "tui", ask.proposed)
                    .ok();
                return;
            }
        }
    })
}

#[tokio::test]
async fn dispatcher_live_role_routed_turn_resolves_via_routing_json() {
    // Given: a routing doc whose `default` role targets claude-opus-4-7
    // while the session's CONFIGURED model is grok-4-6 (also scripted,
    // so choosing it would be visible).
    let (state, log, _dir) = routed_state(
        DISPATCH_ROUTING,
        Duration::from_secs(30),
        vec![
            ("claude-opus-4-7", vec![Step::Turn(usage_turn("routed"))]),
            ("grok-4-6", vec![Step::Turn(usage_turn("configured"))]),
        ],
    );
    // When: one turn runs.
    let capture = drive_turns(&state, "s-role", 1).await;
    // Then: the turn resolved via the routing.json role entry — Usage
    // cites the routed identity at the role's catalog-default effort,
    // and the configured model never composed a stack.
    assert_eq!(capture.finished, 1);
    assert_eq!(
        capture.usage,
        vec![(
            "anthropic".to_owned(),
            "claude-opus-4-7".to_owned(),
            Some("xhigh".to_owned())
        )],
        "usage must cite the routed model, got {:?}",
        capture.usage
    );
    let log = log.lock().expect("log");
    assert_eq!(log.lookups, vec!["claude-opus-4-7".to_owned()]);
    assert_eq!(log.infer_calls, vec!["claude-opus-4-7".to_owned()]);
}

#[tokio::test]
async fn dispatcher_live_stickiness_resume_keeps_pinned_model() {
    // Given: a routed session with two scripted turns on the pinned
    // model (a second turn = the daemon-level resume).
    let (state, log, _dir) = routed_state(
        DISPATCH_ROUTING,
        Duration::from_secs(30),
        vec![
            (
                "claude-opus-4-7",
                vec![Step::Turn(usage_turn("one")), Step::Turn(usage_turn("two"))],
            ),
            ("grok-4-6", vec![Step::Turn(usage_turn("configured"))]),
        ],
    );
    // When: two turns run on the same session.
    let capture = drive_turns(&state, "s-resume", 2).await;
    // Then: both turns cite the pinned launch model (stickiness across
    // resume), and exactly ONE stack lookup happened — the pin never
    // moved, so no second model ever composed.
    assert_eq!(capture.finished, 2);
    assert_eq!(capture.usage.len(), 2);
    assert!(
        capture
            .usage
            .iter()
            .all(|(provider, model, _)| { provider == "anthropic" && model == "claude-opus-4-7" })
    );
    let log = log.lock().expect("log");
    assert_eq!(log.lookups, vec!["claude-opus-4-7".to_owned()]);
    assert_eq!(
        log.infer_calls,
        vec!["claude-opus-4-7".to_owned(), "claude-opus-4-7".to_owned()]
    );
}

#[tokio::test]
async fn dispatcher_live_acknowledged_hop_revalidates_effort_grid() {
    // Given: the routed model hard-fails (outage) and a client answers
    // the emitted FallbackAsk with the proposed hop.
    let (state, log, _dir) = routed_state(
        DISPATCH_ROUTING,
        Duration::from_secs(30),
        vec![
            ("claude-opus-4-7", vec![outage_step()]),
            ("gemini-3-pro", vec![Step::Turn(usage_turn("hopped"))]),
        ],
    );
    let host = state.session("s-hop");
    let attachment = host.attach("tui", None).expect("attach");
    let reader = tokio::spawn(read_turns(attachment.events.live, attachment.control, 1));
    let _answerer = spawn_fallback_answerer(host.mediator());
    // When: one turn runs through the failure and the acknowledged hop.
    let _ = host
        .handle_command(HostCommand::Submit("hello".into()))
        .await;
    let capture = tokio::time::timeout(Duration::from_secs(30), reader)
        .await
        .expect("turn finished")
        .expect("reader task");
    // Then: exactly one ask surfaced, proposing the verbatim chain hop.
    assert_eq!(capture.finished, 1);
    assert_eq!(
        capture.fallback_asks.len(),
        1,
        "asks: {:?}",
        capture.fallback_asks
    );
    assert_eq!(capture.fallback_asks[0].current.model, "claude-opus-4-7");
    assert_eq!(capture.fallback_asks[0].proposed.model, "gemini-3-pro");
    assert_eq!(
        capture.fallback_resolved,
        vec![ModelIdentity::new("anthropic", "gemini-3-pro")]
    );
    // And: the turn was served by the HOP model with the effort grid
    // REVALIDATED — the pinned xhigh rung is off gemini-3-pro's grid,
    // so the dispatch carries gemini's documented default (high).
    assert_eq!(
        capture.usage,
        vec![(
            "anthropic".to_owned(),
            "gemini-3-pro".to_owned(),
            Some("high".to_owned())
        )],
        "hop must revalidate the effort grid, got {:?}",
        capture.usage
    );
    // And: substitution happened ONLY after the acknowledgement — the
    // origin model served first, the hop second.
    let log = log.lock().expect("log");
    assert_eq!(
        log.lookups,
        vec!["claude-opus-4-7".to_owned(), "gemini-3-pro".to_owned()]
    );
    assert_eq!(
        log.infer_calls,
        vec!["claude-opus-4-7".to_owned(), "gemini-3-pro".to_owned()]
    );
}

#[tokio::test]
async fn fallback_no_silent_substitution_unacknowledged_ask_retries_same_model() {
    // Given: the routed model hard-fails EVERY attempt and NO client
    // answers the fallback ask (the Wave-R→Wave-U interim: asks are
    // emitted before every frontend can answer).
    let (state, log, _dir) = routed_state(
        DISPATCH_ROUTING,
        Duration::from_millis(200),
        vec![
            (
                "claude-opus-4-7",
                vec![outage_step(), outage_step(), outage_step()],
            ),
            ("gemini-3-pro", vec![Step::Turn(usage_turn("never"))]),
        ],
    );
    // When: one turn runs with the ask left unacknowledged.
    let capture = drive_turns(&state, "s-unacked", 1).await;
    // Then: the ask surfaced and timed out — the SAME model was retried
    // (two infer calls on the origin), and after the second
    // unacknowledged ask the turn failed typed instead of looping.
    assert_eq!(capture.finished, 1);
    assert_eq!(
        capture.fallback_asks.len(),
        2,
        "asks: {:?}",
        capture.fallback_asks
    );
    assert!(
        capture
            .fallback_asks
            .iter()
            .all(|ask| ask.current.model == "claude-opus-4-7"
                && ask.proposed.model == "gemini-3-pro")
    );
    // ZERO substitution events: the hop model never composed a stack,
    // never served, and no Usage frame cites it.
    let log = log.lock().expect("log");
    assert_eq!(log.lookups, vec!["claude-opus-4-7".to_owned()]);
    assert_eq!(
        log.infer_calls,
        vec!["claude-opus-4-7".to_owned(), "claude-opus-4-7".to_owned()]
    );
    assert!(capture.usage.is_empty(), "usage: {:?}", capture.usage);
    // And: every resolution chose the CURRENT model (deny/cancel/
    // timeout all retry the SAME model — the T9 identity contract).
    assert!(
        capture
            .fallback_resolved
            .iter()
            .all(|chosen| chosen.model == "claude-opus-4-7")
    );
    // And: the v1-decodable degraded-mode notice carries the plan's
    // verbatim wording.
    assert_eq!(
        capture.degraded_retry_notices(),
        2,
        "notifications: {:?}",
        capture.notifications
    );
}

#[tokio::test]
async fn dispatcher_live_unrouted_role_falls_back_to_configured_model() {
    // Given: a routing doc with NO entry for the session role (only the
    // summarizer utility route is configured).
    let (state, log, _dir) = routed_state(
        UNROUTED,
        Duration::from_secs(30),
        vec![
            ("grok-4-6", vec![Step::Turn(usage_turn("configured"))]),
            ("claude-opus-4-7", vec![Step::Turn(usage_turn("never"))]),
        ],
    );
    // When: one turn runs.
    let capture = drive_turns(&state, "s-unrouted", 1).await;
    // Then: the session's CONFIGURED model served the turn explicitly
    // (its own provider identity, no routing effort) — never an
    // implicit hop onto another route.
    assert_eq!(capture.finished, 1);
    assert_eq!(
        capture.usage,
        vec![("xai".to_owned(), "grok-4-6".to_owned(), None)],
        "usage must cite the configured model, got {:?}",
        capture.usage
    );
    let log = log.lock().expect("log");
    assert_eq!(log.lookups, vec!["grok-4-6".to_owned()]);
    assert!(capture.fallback_asks.is_empty());
}
