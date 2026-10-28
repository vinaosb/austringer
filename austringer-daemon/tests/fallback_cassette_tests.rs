//! Fallback-hop acceptance over REAL cassette-backed adapters: the
//! anti-substitution doctrine end-to-end on the production composition
//! path (`ProviderSelection::resolve_with` + `compose_factory`). A typed
//! adapter-visible HTTP 500 outage on the routed model A surfaces the
//! mediated `FallbackAsk`; only the acknowledged hop lands on model B,
//! whose adapter receives request BYTES carrying B's own identity.
//! Complements dispatcher_live_tests (scripted `Inference` stacks, no
//! adapters) and providers/tests/identity_fallback.rs (adapter-internal
//! identity): this file pins the seam BETWEEN them.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use austringer_daemon::attach::HostCommand;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::mediation::PermissionMediator;
use austringer_daemon::protocol::{FallbackAskPayload, ServerMsg, SessionEvent};
use austringer_daemon::provider::{ProviderFamily, ProviderSelection};
use austringer_daemon::routing::{ModelSource, RoutingIntake};
use austringer_daemon::server::DaemonState;
use austringer_daemon::tools::CoreExecutors;
use austringer_daemon::usage::{ComposedInference, InferenceFactory};
use austringer_intel::catalog::Catalog;
use austringer_intel::routing::RoutingTable;
use austringer_providers::identity::ModelIdentity;
use austringer_providers::transport::{
    Body, Transport, TransportFailure, TransportRequest, TransportResponse,
};
use common::cassette::{Cassette, CassetteTransport, anthropic_text_cassette};

/// Routed model A (catalog default effort xhigh) with a one-hop chain
/// onto model B — xhigh is on B's grid too, so an acknowledged hop
/// keeps the pinned rung (dispatch.rs revalidation).
const MODEL_A: &str = "claude-opus-4-7";
const MODEL_B: &str = "claude-sonnet-4-6";

const ROUTING: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["claude-sonnet-4-6"],
  "roles": [ { "role": "default", "model": "claude-opus-4-7" } ],
  "fallbacks": [ { "model": "claude-opus-4-7", "chain": ["claude-sonnet-4-6"] } ],
  "rate_limit_wait_out_secs": 1,
  "retry_attempts": 1
}
"#;

/// The Wave-R→Wave-U degraded-mode sentence (routing.rs): verbatim.
const DEGRADED_RETRY_NOTICE: &str =
    "model fallback asked; no interactive client — retrying same model";

const ANSWER: &str = "cassette hop answer: served by model B";
const PROMPT: &str = "FALLBACK-CASSETTE: does the acknowledged hop reach model B?";

/// Adapter-visible outage: HTTP 500 carrying the Anthropic error-body
/// shape a real overloaded upstream produces. The adapter maps it to
/// the typed retryable `ProviderStreamError::Http`, exhausts the
/// daemon retry budget (3 attempts per infer call), and surfaces a
/// non-rate-limit outage the router classifies onto the fallback chain.
struct OutageCassette {
    requests: Mutex<Vec<TransportRequest>>,
}

impl OutageCassette {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(Vec::new()),
        })
    }

    fn requests(&self) -> Vec<TransportRequest> {
        self.requests.lock().expect("requests").clone()
    }
}

#[async_trait]
impl Transport for OutageCassette {
    async fn send(&self, request: TransportRequest) -> Result<TransportResponse, TransportFailure> {
        if let Ok(mut reqs) = self.requests.lock() {
            reqs.push(request);
        }
        let body =
            br#"{"error":{"type":"api_error","message":"cassette outage: upstream overloaded"}}"#;
        Ok(TransportResponse::with_status(
            500,
            Body::Full(body.to_vec()),
        ))
    }
}

/// One REAL adapter stack per model: the same selection → resolve →
/// compose path the daemon binary drives, over the given transport.
fn cassette_stack(model: &str, transport: Arc<dyn Transport>) -> InferenceFactory {
    let selection = ProviderSelection {
        family: ProviderFamily::Anthropic,
        model: model.to_owned(),
        api_key_env: Some("AUSTRINGER_FALLBACK_CASSETTE_KEY".to_owned()),
        base_url: None,
        inference_timeout: Duration::from_secs(30),
    };
    let resolved = selection
        .resolve_with(|_| Some("fallback-cassette-fake-key".to_owned()))
        .unwrap_or_else(|failure| panic!("resolve failed: {failure}"));
    resolved.compose_factory(transport)
}

/// The routing intake's per-model source: dispatches on the dispatched
/// model id, composing a fresh stack from the matching factory.
fn model_source(a: InferenceFactory, b: InferenceFactory) -> ModelSource {
    Arc::new(move |model: &str| match model {
        MODEL_A => Some(a().inference),
        MODEL_B => Some(b().inference),
        _ => None,
    })
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

/// A routed daemon whose session turns dispatch over the routing table
/// with REAL cassette adapters behind both chain models.
fn routed_cassette_state(
    ask_timeout: Duration,
    outage: Arc<dyn Transport>,
    serving: Arc<dyn Transport>,
) -> (Arc<DaemonState>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = DaemonConfig {
        data_dir: dir.path().to_path_buf(),
        ask_timeout,
        ..DaemonConfig::default()
    };
    let factory: InferenceFactory = Arc::new(|| ComposedInference {
        inference: Box::new(NeverServes),
        identity: Some(ModelIdentity::new("xai", "grok-4-6")),
    });
    let table = RoutingTable::from_json(ROUTING, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("routing doc rejected: {e:?}"));
    let mut state = DaemonState::new(config, factory).expect("daemon state");
    // The shipped in-process CORE_TOOLS executor (what the binary
    // injects): sessions declare their pinned wire tools, so the hop
    // request carries the real tool surface, not the worker default's
    // empty set.
    let executors = Arc::new(CoreExecutors::in_process());
    state.executor_factory = executors.executor_factory();
    state.routing = Some(Arc::new(RoutingIntake {
        table,
        models: model_source(
            cassette_stack(MODEL_A, outage),
            cassette_stack(MODEL_B, serving),
        ),
    }));
    (Arc::new(state), dir)
}

/// What one submitted turn surfaced on the two client lanes (events +
/// usage broadcast; fallback ask/resolved control frames).
#[derive(Default)]
struct TurnCapture {
    usage: Vec<(String, String, Option<String>)>,
    notifications: Vec<(String, String)>,
    asks: Vec<FallbackAskPayload>,
    resolved: Vec<ModelIdentity>,
    outcomes: Vec<String>,
}

fn absorb(msg: ServerMsg, capture: &mut TurnCapture) {
    match msg {
        ServerMsg::Event { frame } => match frame.event {
            SessionEvent::TurnFinished { outcome, .. } => capture.outcomes.push(outcome),
            SessionEvent::Notification { kind, message } => {
                capture.notifications.push((kind, message));
            }
            _ => {}
        },
        ServerMsg::Usage { model, effort, .. } => {
            capture.usage.push((model.provider, model.model, effort));
        }
        ServerMsg::FallbackAsk { ask } => capture.asks.push(ask),
        ServerMsg::FallbackResolved { chosen, .. } => capture.resolved.push(chosen),
        _ => {}
    }
}

/// Drains both lanes until momentarily quiet, so a control frame racing
/// TurnFinished on the other channel (FallbackResolved) is captured
/// before the reader returns.
async fn drain_quiet(
    events: &mut broadcast::Receiver<ServerMsg>,
    control: &mut broadcast::Receiver<ServerMsg>,
    capture: &mut TurnCapture,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let mut received = false;
        while let Ok(msg) = events.try_recv() {
            received = true;
            absorb(msg, capture);
        }
        while let Ok(msg) = control.try_recv() {
            received = true;
            absorb(msg, capture);
        }
        if !received || tokio::time::Instant::now() >= deadline {
            return;
        }
    }
}

/// Reads both lanes until the first TurnFinished (bounded by the
/// caller's outer timeout).
async fn read_turn(
    mut events: broadcast::Receiver<ServerMsg>,
    mut control: broadcast::Receiver<ServerMsg>,
) -> TurnCapture {
    let mut capture = TurnCapture::default();
    loop {
        let msg = tokio::select! {
            msg = events.recv() => msg,
            msg = control.recv() => msg,
        };
        match msg {
            Ok(msg) => {
                let before = capture.outcomes.len();
                absorb(msg, &mut capture);
                if capture.outcomes.len() > before {
                    drain_quiet(&mut events, &mut control, &mut capture).await;
                    return capture;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return capture,
        }
    }
}

/// Submits one prompt on a fresh attachment; returns what the attached
/// client observed (a silent hang becomes a named 30s failure).
async fn drive_turn(state: &DaemonState, session: &str) -> TurnCapture {
    let host = state.session(session);
    let attachment = host.attach("tui", None).expect("attach");
    let reader = tokio::spawn(read_turn(attachment.events.live, attachment.control));
    let _ = host
        .handle_command(HostCommand::Submit(PROMPT.to_owned()))
        .await;
    match tokio::time::timeout(Duration::from_secs(30), reader).await {
        Ok(joined) => joined.expect("reader task"),
        Err(_) => panic!("no TurnFinished within 30s on {session}"),
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
async fn fallback_cassette_acknowledged_hop_serves_retry_through_model_b_adapter() {
    // Given: A's real adapter sees the 500 outage on every attempt; B's
    // real adapter holds a scripted cassette; a client will answer the
    // fallback ask with the proposed hop.
    let outage = OutageCassette::new();
    let serving = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        MODEL_B, ANSWER,
    ))]);
    let (state, _dir) =
        routed_cassette_state(Duration::from_secs(30), outage.clone(), serving.clone());
    let host = state.session("s-cassette-hop");
    let attachment = host.attach("tui", None).expect("attach");
    let reader = tokio::spawn(read_turn(attachment.events.live, attachment.control));
    let _answerer = spawn_fallback_answerer(host.mediator());
    // When: one turn runs through A's outage, the ask, and the ack.
    let _ = host
        .handle_command(HostCommand::Submit(PROMPT.to_owned()))
        .await;
    let capture = tokio::time::timeout(Duration::from_secs(30), reader)
        .await
        .expect("turn finished")
        .expect("reader task");
    // Then: the turn completed, exactly one ask surfaced naming BOTH
    // identities, and the resolution chose the hop.
    assert_eq!(
        capture.outcomes,
        vec!["turn_complete".to_owned()],
        "outcomes: {:?}",
        capture.outcomes
    );
    assert_eq!(capture.asks.len(), 1, "asks: {:?}", capture.asks);
    assert_eq!(
        capture.asks[0].current,
        ModelIdentity::new("anthropic", MODEL_A)
    );
    assert_eq!(
        capture.asks[0].proposed,
        ModelIdentity::new("anthropic", MODEL_B)
    );
    assert_eq!(
        capture.resolved,
        vec![ModelIdentity::new("anthropic", MODEL_B)]
    );
    // And: the hop's Usage frame cites B at the still-on-grid effort.
    assert_eq!(
        capture.usage,
        vec![(
            "anthropic".to_owned(),
            MODEL_B.to_owned(),
            Some("xhigh".to_owned())
        )],
        "usage must cite only the hop model: {:?}",
        capture.usage
    );
    // And (request bytes): A's adapter exhausted its 3-attempt retry
    // budget under A's identity, every attempt carrying the prompt.
    let a_requests = outage.requests();
    assert_eq!(a_requests.len(), 3, "one infer call, retry budget 3");
    for request in &a_requests {
        let body: serde_json::Value =
            serde_json::from_slice(&request.body).expect("A request body JSON");
        assert_eq!(body["model"], MODEL_A, "A's requests carry A's identity");
        assert!(
            String::from_utf8_lossy(&request.body).contains(PROMPT),
            "prompt rides every A attempt"
        );
    }
    // And: B's adapter received EXACTLY the acknowledged retry — one
    // request, B's identity, the prompt, and the family surfaces.
    let b_requests = serving.requests();
    assert_eq!(b_requests.len(), 1, "B serves exactly the hop retry");
    let body: serde_json::Value =
        serde_json::from_slice(&b_requests[0].body).expect("B request body JSON");
    assert_eq!(
        body["model"], MODEL_B,
        "B's request must carry B's identity"
    );
    assert!(
        body["messages"].to_string().contains(PROMPT),
        "the submitted prompt must ride the hop: {}",
        body["messages"]
    );
    assert!(
        body["system"].as_array().is_some_and(|s| !s.is_empty()),
        "system surface rides the hop"
    );
    assert!(
        body["tools"].as_array().is_some_and(|t| !t.is_empty()),
        "tools surface rides the hop"
    );
    assert_eq!(
        b_requests[0].url, "https://api.anthropic.com/v1/messages",
        "B composes the documented family endpoint"
    );
    assert_eq!(
        b_requests[0].headers.get("x-api-key").map(String::as_str),
        Some("fallback-cassette-fake-key"),
        "the resolved key rides the hop request"
    );
}

#[tokio::test]
async fn fallback_cassette_unacknowledged_ask_retries_same_model_and_never_touches_b() {
    // Given: A's adapter sees the outage on every attempt; B's adapter
    // holds a cassette; NO client answers the ask (200ms ask timeout).
    let outage = OutageCassette::new();
    let serving = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        MODEL_B, ANSWER,
    ))]);
    let (state, _dir) =
        routed_cassette_state(Duration::from_millis(200), outage.clone(), serving.clone());
    // When: one turn runs with the ask left unacknowledged.
    let capture = drive_turn(&state, "s-cassette-unacked").await;
    // Then: two asks surfaced (MAX_UNACKNOWLEDGED_ASKS=1 bounds the
    // cycle), every one proposing the same A→B hop.
    assert_eq!(capture.asks.len(), 2, "asks: {:?}", capture.asks);
    assert!(
        capture
            .asks
            .iter()
            .all(|ask| ask.current.model == MODEL_A && ask.proposed.model == MODEL_B)
    );
    // And: the turn failed typed — never a B-served completion.
    assert_eq!(capture.outcomes.len(), 1);
    assert!(
        capture.outcomes[0].starts_with("failed"),
        "outcome: {:?}",
        capture.outcomes
    );
    // And: ZERO substitution — B's adapter never received a request.
    assert!(
        serving.requests().is_empty(),
        "B must never be contacted: {:?}",
        serving.requests()
    );
    // And: the SAME model retried through A's real adapter — two infer
    // calls x the 3-attempt budget, every request under A's identity.
    let a_requests = outage.requests();
    assert_eq!(
        a_requests.len(),
        6,
        "same-model retry over the real adapter"
    );
    for request in &a_requests {
        let body: serde_json::Value =
            serde_json::from_slice(&request.body).expect("A request body JSON");
        assert_eq!(body["model"], MODEL_A);
    }
    // And: no Usage frame cites any model, and the v1-decodable
    // degraded-mode notice is verbatim, once per unacknowledged ask.
    assert!(capture.usage.is_empty(), "usage: {:?}", capture.usage);
    let notices = capture
        .notifications
        .iter()
        .filter(|(_, message)| message == DEGRADED_RETRY_NOTICE)
        .count();
    assert_eq!(notices, 2, "notifications: {:?}", capture.notifications);
}
