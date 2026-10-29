//! The axum surface: WebSocket attach, SSE events, JSON commands,
//! worker connections, web assets, and the reload endpoint — all
//! behind the hardened loopback auth middleware (peer address +
//! defensive Host rejection + generated token; Host is NEVER an auth
//! factor).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderName, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

use serde::Deserialize;

use crate::attach::SessionHost;
use crate::auth::{self, AuthVerdict};
use crate::config::DaemonConfig;
use crate::stages::WorkerToolExecutor;
use crate::tools::ExecutorFactory;
use crate::usage::{ComposedInference, InferenceFactory, IntoSessionFactory};

use crate::worker::WorkerRouter;

/// Shared daemon state.
pub struct DaemonState {
    /// Effective config.
    pub config: DaemonConfig,
    /// The live auth token.
    pub token: String,
    /// Inference factory for new sessions (scripted in tests, provider
    /// adapter in the binary); yields each session's stack WITH its
    /// model identity so the session can emit Usage frames.
    pub inference_factory: InferenceFactory,
    /// Submit-time projection intake factory for new sessions (M10):
    /// composes each session's mediation candidates against its session
    /// dir. The default keeps the submit path marker-only; the binary
    /// wires the provider + T42 routing seam.
    pub projection_factory: crate::projection::IntakeFactory,
    /// Tool-execution stage factory for new sessions (todo 3,
    /// ecosystem alignment). The default keeps the split-brain worker
    /// executor (out-of-process execution, worker protocol preserved);
    /// the binary injects the in-process CORE_TOOLS registry factory.
    pub executor_factory: ExecutorFactory,
    /// The dispatcher intake for new sessions (todo 9, ecosystem
    /// alignment): routing table + per-model provider stacks. When
    /// present (and the session factory is identity-bearing), every
    /// session turn dispatches through austringer-intel's `Dispatcher` —
    /// role routes from routing.json, stickiness pins, and mediated
    /// ask-first fallbacks. `None` (default) keeps the direct
    /// session-model path.
    pub routing: Option<Arc<crate::routing::RoutingIntake>>,
    /// Session hosts by URL-addressable session id.
    pub hosts: Mutex<HashMap<String, Arc<SessionHost>>>,
    /// Per-session cost ledgers keyed by session id, OUTLIVING host
    /// eviction: cumulative spend is a session-lifetime contract, so
    /// an evicted-and-recreated host resumes against the SAME ledger
    /// (the cost-cap thresholds survive eviction). Entries drop when
    /// retention GC removes the session itself.
    pub ledgers: Mutex<HashMap<String, Arc<crate::cost_ledger::SessionLedger>>>,
    /// Per-session prompt epochs keyed by session id, OUTLIVING host
    /// eviction like the ledgers: the PromptBuilder baseline (blake3
    /// stable-prefix chain + the cache-violation tracker) records at
    /// session creation, and a re-created host's re-composition is
    /// compared against it — drift surfaces as a typed transcript
    /// notice, never a silent prefix-cache churn. In-memory only.
    pub prompt_epochs: Mutex<HashMap<String, crate::prompt::PromptEpoch>>,
    /// Engine session dirs root.
    pub sessions_root: PathBuf,
    /// Remote-worker router (split-brain tool execution).
    pub worker_router: Arc<WorkerRouter>,
    /// The startup-loaded skills surface (todo 6): every session's
    /// submit lane routes through its trust/consent gates.
    pub skills: Arc<crate::skills::SkillsRuntime>,
    /// Launch services for `/launch` child sessions (todo 8): the
    /// same injected factories and permission policy the session
    /// factory uses, rooted at the sessions dir.
    pub launch_services: Arc<crate::launch::LaunchServices>,
    /// The bound listener kept for zero-downtime reload handoff.
    pub listener: Mutex<Option<std::net::TcpListener>>,
    /// Fired when a reload completed handing the socket off.
    pub drain: tokio::sync::watch::Sender<bool>,
}

impl DaemonState {
    /// Build state (generating a token when the config has none).
    /// Accepts an [`InferenceFactory`] or a legacy identity-less
    /// factory (adapted; those sessions emit no Usage frames).
    ///
    /// # Errors
    /// token generation failure.
    pub fn new(
        config: DaemonConfig,
        inference_factory: impl IntoSessionFactory,
    ) -> Result<Self, auth::AuthError> {
        let token = match &config.token {
            Some(fixed) => fixed.clone(),
            None => auth::generate_token()?,
        };
        let worker_router = Arc::new(WorkerRouter::new(config.worker_call_timeout));
        let skills = Arc::new(crate::skills::SkillsRuntime::load(&config.data_dir));
        let (drain, _) = tokio::sync::watch::channel(false);
        let executor_factory: ExecutorFactory = Arc::new({
            let worker_router = Arc::clone(&worker_router);
            move |_session_dir, event_sink| {
                // The worker protocol has no in-process registry, so it
                // declares nothing on the wire.
                (
                    Box::new(WorkerToolExecutor::new(
                        Arc::clone(&worker_router),
                        None,
                        event_sink,
                    )),
                    Vec::new(),
                )
            }
        });
        let inference_factory = inference_factory.into_factory();
        let launch_services = Arc::new(crate::launch::LaunchServices {
            inference_factory: Arc::clone(&inference_factory),
            executor_factory: Arc::clone(&executor_factory),
            sessions_root: config.data_dir.join("sessions"),
            ask_policy: config.ask_policy.clone(),
            ask_timeout: config.ask_timeout,
        });
        Ok(Self {
            token,
            worker_router,
            skills,
            launch_services,
            sessions_root: config.data_dir.join("sessions"),
            config,
            inference_factory,
            projection_factory: crate::projection::disabled_factory(),
            executor_factory,
            routing: None,
            hosts: Mutex::new(HashMap::new()),
            ledgers: Mutex::new(HashMap::new()),
            prompt_epochs: Mutex::new(HashMap::new()),
            listener: Mutex::new(None),
            drain,
        })
    }

    /// Create (or return) a session host with default loop settings;
    /// the id is the stable, deep-linkable URL segment.
    pub fn session(&self, id: &str) -> Arc<SessionHost> {
        self.session_with_config(id, austringer_core::agent_loop::LoopConfig::default())
    }

    /// Create (or return) a SCHEDULED session host: per-automation
    /// turn budgets (`loop_config.max_turns`) bound the unattended run.
    pub fn session_scheduled(&self, id: &str, turn_budget: Option<u32>) -> Arc<SessionHost> {
        let mut config = austringer_core::agent_loop::LoopConfig::default();
        if let Some(max_turns) = turn_budget {
            config.max_turns = max_turns;
        }
        self.session_with_config(id, config)
    }

    fn session_with_config(
        &self,
        id: &str,
        loop_config: austringer_core::agent_loop::LoopConfig,
    ) -> Arc<SessionHost> {
        let mut hosts = match self.hosts.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(host) = hosts.get(id) {
            return Arc::clone(host);
        }
        let (host, cmd_rx) = SessionHost::new(
            id,
            self.config.ring_capacity,
            self.config.broadcast_capacity,
            self.config.max_clients_per_session,
            self.config.ask_policy.clone(),
            self.config.ask_timeout,
            if self.config.durable {
                Some(self.config.data_dir.clone())
            } else {
                None
            },
        );
        let session_dir = self.sessions_root.join(id);
        let _ = std::fs::create_dir_all(&session_dir);
        // The projection intake must be set BEFORE the engine spawns:
        // the steer stage captures its notice queue at stage construction.
        let _ = host.media_dir.set(session_dir.clone());
        let _ = host.sessions_store_root.set(self.config.data_dir.clone());
        let _ = host.journal_max_bytes.set(self.config.journal_max_bytes);
        let _ = host.skills.set(Arc::clone(&self.skills));
        let _ = host.launcher.set(Arc::clone(&self.launch_services));
        // Todo 33: the per-session cost ledger binds before the engine
        // spawns — the submit intake gate and HUD consumers read it, and
        // the inference decorator below enforces it (preflight halt +
        // threshold events). Scheduled sessions get the same daemon-wide
        // cap: an automation owns its ledger exactly like a live one.
        // The ledger lives in the daemon-level map (eviction survival):
        // a re-created host resumes against the same cumulative spend.
        let cost_ledger = {
            let mut ledgers = match self.ledgers.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            Arc::clone(ledgers.entry(id.to_owned()).or_insert_with(|| {
                Arc::new(crate::cost_ledger::SessionLedger::new(
                    self.config.session_cost_cap_micros,
                ))
            }))
        };
        let _ = host.cost_ledger.set(Arc::clone(&cost_ledger));
        if let Some(intake) = (self.projection_factory)(&session_dir) {
            let _ = host.projection.set(intake);
        }
        let (executor, wire_tools) = (self.executor_factory)(&session_dir, host.event_sink());
        let ComposedInference {
            inference,
            identity,
        } = (self.inference_factory)();
        // The session's model identity for the same-model condenser
        // seam (todo 13); identity-less scripted stacks have none.
        let session_model = identity.as_ref().map(|identity| {
            austringer_core::compaction::condense::ModelId::new(format!(
                "{}/{}",
                identity.provider, identity.model
            ))
        });
        // Identity-bearing stacks (the shipped provider composition)
        // emit one Usage frame per assistant turn; identity-less
        // scripted stacks stay bare. The cost ledger wraps the
        // Usage-accounted stack: its preflight gate halts further
        // provider calls once the cap is reached, and priced spend
        // books into the session ledger (todo 33). With a routing
        // intake bound (todo 9), the SessionRouter OWNS that wrap per
        // dispatched model instead — identity, Usage frames, and spend
        // follow the dispatch, and an acknowledged fallback hop
        // re-cites the new model.
        let inference = match (&self.routing, identity) {
            (Some(intake), Some(configured)) => Box::new(crate::routing::SessionRouter::new(
                Arc::clone(intake),
                id,
                Arc::clone(&host),
                Arc::clone(&cost_ledger),
                configured,
            ))
                as Box<dyn austringer_core::agent_loop::Inference>,
            (None, Some(identity)) => {
                let usage = crate::usage::UsageInference::new(
                    Arc::clone(&host),
                    identity.clone(),
                    None,
                    inference,
                );
                Box::new(crate::cost_ledger::CostLedgerInference::new(
                    Arc::clone(&host),
                    cost_ledger,
                    identity,
                    Box::new(usage),
                ))
            }
            (_, None) => inference,
        };
        // Todo 13: the dormant compaction engine, wired per session.
        // The composed stack is shared between the loop and the
        // same-model condenser (identity-less stacks condense through
        // the deterministic fallback). Bound BEFORE the stage-set
        // builder reads it.
        let shared = crate::compaction::SharedStack::new(inference);
        let condenser = session_model
            .as_ref()
            .map(|model| shared.condenser(model.clone()));
        let compaction = Arc::new(crate::compaction::SessionCompaction::new(
            session_dir.clone(),
            &self.config.compaction,
            session_model.unwrap_or_else(|| {
                austringer_core::compaction::condense::ModelId::new("austringer/unconfigured")
            }),
            condenser,
            Arc::downgrade(&host),
        ));
        let _ = host.compaction.set(compaction);
        let _ = host.snapshot_keep_last.set(self.config.snapshot_keep_last);
        // The wire surfaces ride the loop config: the stable system
        // prompt plus the session registry's pinned core schemas
        // (research §24.6/§33.1). Set once per session — never churned
        // per request — so the provider prefix cache survives turns.
        // The prompt composes ONCE here through the core
        // PromptBuilder/ContextSpec surface (byte-identical to the
        // legacy compose_system_prompt contract, pinned by
        // prompt_composition_tests) over the daemon's working
        // directory (the same root the LSP and run_tests tools use)
        // and is frozen for the session's lifetime; the recorded
        // epoch makes any later re-composition drift observable — a
        // `prompt_cache` notice on the hub, never a silent churn.
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut loop_config = loop_config;
        let (system, prompt_notices) = crate::prompt::bind_system_prompt(
            &self.prompt_epochs,
            id,
            &workspace_root,
            &self.config.data_dir.join("ast-cache"),
            self.config.repomap_token_budget,
        );
        for message in prompt_notices {
            host.publish(crate::events::SessionEvent::Notification {
                kind: crate::prompt::PROMPT_CACHE_KIND.to_owned(),
                message,
            });
        }
        loop_config.system = system;
        loop_config.tools = wire_tools;
        let _ = host.spawn_engine_with_config(
            session_dir,
            host.standard_stages(executor),
            shared.handle(),
            cmd_rx,
            loop_config,
        );
        hosts.insert(id.to_owned(), Arc::clone(&host));
        host
    }

    /// All live session ids.
    #[must_use]
    pub fn session_ids(&self) -> Vec<String> {
        match self.hosts.lock() {
            Ok(guard) => guard.keys().cloned().collect(),
            Err(poisoned) => poisoned.into_inner().keys().cloned().collect(),
        }
    }

    /// Look up a live session host WITHOUT creating one (read-only
    /// routes and notification resolvers; unknown ids stay unknown).
    #[must_use]
    pub fn try_session(&self, id: &str) -> Option<Arc<SessionHost>> {
        match self.hosts.lock() {
            Ok(guard) => guard.get(id).cloned(),
            Err(poisoned) => poisoned.into_inner().get(id).cloned(),
        }
    }

    /// Register the bound listener for reload handoff.
    pub fn register_listener(&self, listener: std::net::TcpListener) {
        match self.listener.lock() {
            Ok(mut guard) => *guard = Some(listener),
            Err(poisoned) => *poisoned.into_inner() = Some(listener),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct TokenQuery {
    pub(crate) after: Option<u64>,
    /// SSE wire-version opt-in: the event stream carries no Hello, so
    /// v2 (typed media frames + media results) is opt-in via
    /// `?version=2`; anything else serves the v1 downgrade.
    pub(crate) version: Option<u32>,
}

fn query_token(uri: &axum::http::Uri) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&')
            .find_map(|pair| pair.strip_prefix("token=").map(str::to_owned))
    })
}

fn presented_token(headers: &axum::http::HeaderMap, query: Option<&str>) -> Option<String> {
    if let Some(token) = query {
        return Some(token.to_owned());
    }
    if let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(token.to_owned());
    }
    headers
        .get(HeaderName::from_static("x-austringer-token"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// Public (pre-auth) routes: the SPA shell and assets, including the
/// two-segment session deep-link page (content routes under
/// `/s/{sid}/…` stay authed).
fn is_public(path: &str) -> bool {
    if path == "/" || path == "/favicon.ico" || path.starts_with("/assets/") {
        return true;
    }
    if let Some(rest) = path.strip_prefix("/s/") {
        return !rest.is_empty() && !rest.contains('/');
    }
    false
}

async fn auth_mw(
    State(state): State<Arc<DaemonState>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    if is_public(&path) {
        return next.run(request).await;
    }
    let host_header = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let presented = presented_token(request.headers(), query_token(request.uri()).as_deref());
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(
            || SocketAddr::from(([127, 0, 0, 1], 0)),
            |ConnectInfo(addr)| *addr,
        );
    let verdict = auth::check(
        peer,
        host_header.as_deref(),
        &state.token,
        presented.as_deref(),
    );
    match verdict {
        AuthVerdict::Allowed => next.run(request).await,
        AuthVerdict::RefusedHost => forbidden("host header refused"),
        AuthVerdict::RefusedPeer => forbidden("peer refused"),
        AuthVerdict::RefusedNoToken | AuthVerdict::RefusedTokenMismatch => unauthorized(),
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(HeaderName::from_static("www-authenticate"), "Bearer")],
        "unauthorized",
    )
        .into_response()
}

fn forbidden(reason: &'static str) -> Response {
    (StatusCode::FORBIDDEN, reason).into_response()
}

/// Build the full daemon router (bind loopback; serve with
/// `into_make_service_with_connect_info`).
pub fn build_router(state: Arc<DaemonState>) -> Router {
    Router::new()
        .route("/", get(crate::handlers::root))
        .route("/sessions", get(crate::handlers::list_sessions))
        .route("/sessions/search", get(crate::search::sessions_search))
        .route("/s/{sid}", get(crate::handlers::session_page))
        .route("/s/{sid}/attach", get(crate::handlers::attach_ws))
        .route("/s/{sid}/events", get(crate::handlers::sse_events))
        .route("/s/{sid}/history", get(crate::handlers::session_history))
        .route("/s/{sid}/cmd", post(crate::handlers::post_command))
        .route("/s/{sid}/media", post(crate::media::upload_media))
        .route("/s/{sid}/media/{hash}", get(crate::media::get_media))
        .route("/automations", get(crate::handlers::list_automations))
        .route("/packs", get(crate::handlers::list_packs))
        .route("/worker", get(crate::ws::worker_ws))
        .route("/admin/reload", post(crate::handlers::reload_endpoint))
        .layer(middleware::from_fn_with_state(Arc::clone(&state), auth_mw))
        .with_state(state)
}
