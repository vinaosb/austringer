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

use harness_core::agent_loop::Inference;

use crate::attach::SessionHost;
use crate::auth::{self, AuthVerdict};
use crate::config::DaemonConfig;

use crate::stages::WorkerToolExecutor;

use crate::worker::WorkerRouter;

/// Shared daemon state.
pub struct DaemonState {
    /// Effective config.
    pub config: DaemonConfig,
    /// The live auth token.
    pub token: String,
    /// Inference factory for new sessions (scripted in tests, provider
    /// adapter in the binary).
    pub inference_factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>,
    /// Session hosts by URL-addressable session id.
    pub hosts: Mutex<HashMap<String, Arc<SessionHost>>>,
    /// Engine session dirs root.
    pub sessions_root: PathBuf,
    /// Remote-worker router (split-brain tool execution).
    pub worker_router: Arc<WorkerRouter>,
    /// The bound listener kept for zero-downtime reload handoff.
    pub listener: Mutex<Option<std::net::TcpListener>>,
    /// Fired when a reload completed handing the socket off.
    pub drain: tokio::sync::watch::Sender<bool>,
}

impl DaemonState {
    /// Build state (generating a token when the config has none).
    ///
    /// # Errors
    /// token generation failure.
    pub fn new(
        config: DaemonConfig,
        inference_factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>,
    ) -> Result<Self, auth::AuthError> {
        let token = match &config.token {
            Some(fixed) => fixed.clone(),
            None => auth::generate_token()?,
        };
        let worker_router = Arc::new(WorkerRouter::new(config.worker_call_timeout));
        let (drain, _) = tokio::sync::watch::channel(false);
        Ok(Self {
            token,
            worker_router,
            sessions_root: config.data_dir.join("sessions"),
            config,
            inference_factory,
            hosts: Mutex::new(HashMap::new()),
            listener: Mutex::new(None),
            drain,
        })
    }

    /// Create (or return) a session host with default loop settings;
    /// the id is the stable, deep-linkable URL segment.
    pub fn session(&self, id: &str) -> Arc<SessionHost> {
        self.session_with_config(id, harness_core::agent_loop::LoopConfig::default())
    }

    /// Create (or return) a SCHEDULED session host: per-automation
    /// turn budgets (`loop_config.max_turns`) bound the unattended run.
    pub fn session_scheduled(&self, id: &str, turn_budget: Option<u32>) -> Arc<SessionHost> {
        let mut config = harness_core::agent_loop::LoopConfig::default();
        if let Some(max_turns) = turn_budget {
            config.max_turns = max_turns;
        }
        self.session_with_config(id, config)
    }

    fn session_with_config(
        &self,
        id: &str,
        loop_config: harness_core::agent_loop::LoopConfig,
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
        let executor =
            WorkerToolExecutor::new(Arc::clone(&self.worker_router), None, host.event_sink());
        let _ = host.spawn_engine_with_config(
            session_dir.clone(),
            host.standard_stages(executor),
            (self.inference_factory)(),
            cmd_rx,
            loop_config,
        );
        let _ = host.media_dir.set(session_dir);
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
        .get(HeaderName::from_static("x-harness-token"))
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
        .route("/s/{sid}", get(crate::handlers::session_page))
        .route("/s/{sid}/attach", get(crate::handlers::attach_ws))
        .route("/s/{sid}/events", get(crate::handlers::sse_events))
        .route("/s/{sid}/cmd", post(crate::handlers::post_command))
        .route("/s/{sid}/media", post(crate::media::upload_media))
        .route("/s/{sid}/media/{hash}", get(crate::media::get_media))
        .route("/worker", get(crate::ws::worker_ws))
        .route("/admin/reload", post(crate::handlers::reload_endpoint))
        .layer(middleware::from_fn_with_state(Arc::clone(&state), auth_mw))
        .with_state(state)
}
