//! Multi-client session hosting: one [`SessionHost`] per session owns
//! the event hub, the permission mediator, and the client registry.
//! Every attached client reads the same hub, so two clients see
//! byte-identical event streams. The engine driver (real austringer-core
//! `AgentLoop` over mediated approval + split-brain execution + shared
//! steering) lives in [`crate::engine`]; the submit-intake gates
//! (cost cap, skills, media, `/launch`) in [`intake`].

mod idle;
mod intake;

pub(crate) use idle::InFlightTurn;

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, mpsc};

use austringer_core::event::{ContentBlock, MediaRef};

use crate::events::{EventHub, ReplayKind, Subscription};
use crate::mediation::{MediationPolicy, PermissionMediator, control_bus};
use crate::protocol::{Frame, ServerMsg, SessionEvent};

/// Engine-driver commands. (No `Eq`: content blocks carry strings only
/// compared by `PartialEq`.)
#[derive(Debug, Clone, PartialEq)]
pub enum HostCommand {
    /// Start a new turn with this prompt.
    Submit(String),
    /// Start a new turn carrying previously uploaded media (intake
    /// form; resolved to blocks before queueing).
    SubmitMedia {
        /// User text (may be empty when the turn is media-only).
        text: String,
        /// Stored media references from the upload route.
        media: Vec<MediaRef>,
    },
    /// Start a new turn with typed content blocks (resolved form).
    SubmitBlocks(Vec<ContentBlock>),
    /// Steer the running turn (signal-with-start: journaled, queued to
    /// the live turn, or starts one when idle).
    Steer(String),
    /// Cancel the running turn.
    Cancel,
}

/// Why a submit was rejected at intake (typed, surfaced to the
/// submitting connection).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandRejected {
    /// A media reference named nothing in the session store.
    #[error("unknown media ref {0}")]
    UnknownMedia(String),
    /// The submit carried no content at all.
    #[error("empty submit")]
    Empty,
    /// A skill activation (explicit `/skill` or trigger-driven) was
    /// refused by the trust gate; the refusal carries the tier.
    #[error("{0}")]
    SkillRefused(crate::skills::SkillRefusal),
    /// A `/launch` request was refused before any child spawned; the
    /// refusal carries the typed launch reason.
    #[error("{0}")]
    LaunchRefused(crate::launch::LaunchRefusal),
    /// The session's cost cap is reached: further turns are refused
    /// with the typed stop reason (names consumed and cap). A terminal
    /// cost stop — never a silent drop, never a model switch.
    #[error("{0}")]
    CostCapReached(austringer_core::budget::StopReason),
    /// The host was evicted (idle) before this command landed; the
    /// session re-creates on the next touch — retry. Never a silent
    /// drop.
    #[error("session evicted; retry")]
    Evicted,
}

/// Attach failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttachError {
    /// The session's client cap is reached (reconnect-storm bound).
    #[error("session client cap reached")]
    TooManyClients,
    /// The host was evicted (idle) mid-handshake; the next touch
    /// re-creates it — reconnect.
    #[error("session evicted; reconnect")]
    Evicted,
}

/// What a client gets when it attaches.
pub struct Attachment {
    /// Bounded replay + live events from the hub.
    pub events: Subscription,
    /// Permission asks and resolutions.
    pub control: broadcast::Receiver<ServerMsg>,
    /// Highest event id at attach time.
    pub last_event_id: u64,
    /// The replay plan (delta vs resync) for transports.
    pub replay: ReplayKind,
}

/// Mid-run steering queue shared between the daemon signal path and the
/// loop's Steer stage.
pub type SteerQueue = Arc<Mutex<VecDeque<String>>>;

/// Per-session host.
pub struct SessionHost {
    session_id: String,
    hub: Mutex<EventHub>,
    control: broadcast::Sender<ServerMsg>,
    mediator: Arc<PermissionMediator>,
    clients: Mutex<HashSet<String>>,
    max_clients: usize,
    /// The engine-driver command lane; taken (dropped) by idle
    /// eviction so the driver's `recv()` ends and the engine task —
    /// with its `AgentLoop` — terminates.
    cmd_tx: Mutex<Option<mpsc::Sender<HostCommand>>>,
    /// The executor event lane; taken alongside `cmd_tx` (the
    /// executor's own clone lives inside the engine's stage set, so
    /// the sink pump ends only after the engine task drops it).
    event_sink_tx: Mutex<Option<mpsc::Sender<SessionEvent>>>,
    steer_queue: SteerQueue,
    /// Session directory (media store root for submit resolution);
    /// set once by the session factory after construction.
    pub(crate) media_dir: std::sync::OnceLock<PathBuf>,
    /// Submit-time projection intake (plan M10); set once by the
    /// session factory BEFORE the engine spawns (the steer stage
    /// captures its notice queue).
    pub(crate) projection: std::sync::OnceLock<Arc<crate::projection::ProjectionIntake>>,
    /// The daemon-wide skills runtime (todo 6); set once by the
    /// session factory. Every Submit passes its trust/consent gates
    /// (`/skill` commands and trigger matches alike).
    pub(crate) skills: std::sync::OnceLock<Arc<crate::skills::SkillsRuntime>>,
    /// The daemon-wide launch services (todo 8); set once by the
    /// session factory. `/launch` submits fail closed when unbound.
    pub(crate) launcher: std::sync::OnceLock<Arc<crate::launch::LaunchServices>>,
    /// The session cost ledger (todo 33); set once by the session
    /// factory BEFORE the engine spawns (the intake gate and HUD
    /// consumers read it). Unset on hosts built outside the factory —
    /// those sessions have no cost surface (honest absence, fail-open:
    /// nothing to trip).
    pub(crate) cost_ledger: std::sync::OnceLock<Arc<crate::cost_ledger::SessionLedger>>,
    /// Sessions-store root (data dir); set once by the session factory.
    /// The engine's live feed appends each turn's recorded events to the
    /// store so `sessions search` covers live sessions. Unset on hosts
    /// built outside the factory — honest absence (nothing persisted).
    pub(crate) sessions_store_root: std::sync::OnceLock<PathBuf>,
    /// The todo-13 compaction service (auto threshold + `/condense`);
    /// set once by the session factory BEFORE the engine spawns (the
    /// stage-set builder reads it). Unset on hosts built outside the
    /// factory — honest absence (no compaction surface).
    pub(crate) compaction: std::sync::OnceLock<Arc<crate::compaction::SessionCompaction>>,
    /// Snapshot GC bound for turn-completion checkpoint recording; set
    /// once by the session factory. Unset hosts fall back to the
    /// conservative default.
    pub(crate) snapshot_keep_last: std::sync::OnceLock<usize>,
    /// The interactive plan-mode gate (todo 16); inactive by default.
    plan: crate::plan::PlanGate,
    pub(crate) turn_counter: AtomicU64,
    /// Unix-millis of the last turn activity (the eviction idleness
    /// clock); set at construction and at each turn start.
    pub(crate) last_activity_ms: AtomicU64,
    /// An engine turn is running (RAII-marked in the driver; blocks
    /// idle eviction so an active turn always completes).
    pub(crate) turn_in_flight: AtomicBool,
    /// The engine driver task's loop ended — the observable that an
    /// eviction actually freed the engine.
    pub(crate) engine_task_done: AtomicBool,
    /// Channels were taken by eviction; intake and attach refuse
    /// typed instead of silently dropping.
    pub(crate) evicted: AtomicBool,
    pub(crate) journal_root: Option<PathBuf>,
    /// Turn-journal rotation cap (`DaemonConfig::journal_max_bytes`);
    /// set once by the session factory. Hosts built outside the
    /// factory keep the 32 MiB default (see [`SessionHost::journal_max_bytes`]).
    pub(crate) journal_max_bytes: std::sync::OnceLock<u64>,
}

impl SessionHost {
    /// Build a host (and its command receiver) for one session.
    #[must_use]
    pub fn new(
        session_id: &str,
        ring_capacity: usize,
        broadcast_capacity: usize,
        max_clients: usize,
        policy: MediationPolicy,
        ask_timeout: std::time::Duration,
        journal_root: Option<PathBuf>,
    ) -> (Arc<Self>, mpsc::Receiver<HostCommand>) {
        let control = control_bus(broadcast_capacity);
        let mediator = Arc::new(PermissionMediator::new(
            policy,
            ask_timeout,
            control.clone(),
        ));
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        // Bounded daemon-event lane (diffs); overflow drops are safe —
        // diffs are advisory, the hub stays authoritative.
        let (event_sink_tx, event_sink_rx) = mpsc::channel(16);
        let steer_queue: SteerQueue = Arc::new(Mutex::new(VecDeque::new()));
        let host = Arc::new(Self {
            session_id: session_id.to_owned(),
            hub: Mutex::new(EventHub::new(ring_capacity, broadcast_capacity)),
            control,
            mediator,
            clients: Mutex::new(HashSet::new()),
            max_clients,
            cmd_tx: Mutex::new(Some(cmd_tx)),
            event_sink_tx: Mutex::new(Some(event_sink_tx)),
            steer_queue: Arc::clone(&steer_queue),
            media_dir: std::sync::OnceLock::new(),
            projection: std::sync::OnceLock::new(),
            skills: std::sync::OnceLock::new(),
            launcher: std::sync::OnceLock::new(),
            cost_ledger: std::sync::OnceLock::new(),
            sessions_store_root: std::sync::OnceLock::new(),
            compaction: std::sync::OnceLock::new(),
            snapshot_keep_last: std::sync::OnceLock::new(),
            plan: crate::plan::PlanGate::default(),
            turn_counter: AtomicU64::new(0),
            last_activity_ms: AtomicU64::new(0),
            turn_in_flight: AtomicBool::new(false),
            engine_task_done: AtomicBool::new(false),
            evicted: AtomicBool::new(false),
            journal_root,
            journal_max_bytes: std::sync::OnceLock::new(),
        });
        host.mark_activity();
        host.pump_event_sink(event_sink_rx);
        (host, cmd_rx)
    }

    fn pump_event_sink(self: &Arc<Self>, mut rx: mpsc::Receiver<SessionEvent>) {
        let host = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                host.publish(event);
            }
        });
    }

    /// The session id (stable, URL-addressable).
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// The shared mediator (transports route answers/cancels here).
    #[must_use]
    pub fn mediator(&self) -> Arc<PermissionMediator> {
        Arc::clone(&self.mediator)
    }

    /// The shared steer queue (mid-run signal delivery).
    #[must_use]
    pub fn steer_queue(&self) -> SteerQueue {
        Arc::clone(&self.steer_queue)
    }

    /// The turn-journal rotation cap threaded from
    /// `DaemonConfig::journal_max_bytes`; hosts built outside the
    /// session factory keep the 32 MiB default.
    pub(crate) fn journal_max_bytes(&self) -> u64 {
        self.journal_max_bytes
            .get()
            .copied()
            .unwrap_or(crate::journal::DEFAULT_JOURNAL_MAX_BYTES)
    }

    /// The session cost ledger (todo 33): the badge-state seam HUD
    /// surfaces consume (`snapshot()`); `None` on hosts built outside
    /// the daemon factory (no cost surface).
    #[must_use]
    pub fn cost_ledger(&self) -> Option<Arc<crate::cost_ledger::SessionLedger>> {
        self.cost_ledger.get().cloned()
    }

    /// The interactive plan-mode gate (todo 16): OFF by default; the
    /// approval/execution stage wrappers and the `PlanMode` transports
    /// all consult this one state.
    #[must_use]
    pub fn plan_gate(&self) -> &crate::plan::PlanGate {
        &self.plan
    }

    fn lock_clients(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        match self.clients.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn with_hub<T>(&self, f: impl FnOnce(&mut EventHub) -> T) -> T {
        let mut hub = match self.hub.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut hub)
    }

    /// Publish one daemon-level event (monotonic id assigned here).
    pub fn publish(&self, event: SessionEvent) -> Frame {
        self.with_hub(|hub| hub.publish(event))
    }

    /// Fan out one transient server message to live subscribers (no
    /// event id, no ring retention) — the v2-only receipt lane
    /// (MediaResult, Usage, Verdict). v1 connections receive nothing:
    /// the per-connection downgrade withholds v2-only variants at the
    /// transport pump.
    pub fn broadcast(&self, msg: ServerMsg) {
        self.with_hub(|hub| hub.broadcast(msg));
    }

    /// Mediate a model-fallback ask (T9: routing proposes, the user
    /// disposes). The `FallbackAsk` rides the control bus to v2
    /// clients while a v1-decodable `Notification` pair rides the hub,
    /// so no v1 client silently misses an ask; resolution flows through
    /// the permission mediator under the session's policy and the
    /// returned identity is what the next request targets (deny,
    /// cancel, and timeout all retry the SAME model).
    pub async fn mediate_fallback(
        &self,
        request: austringer_providers::identity::FallbackRequest,
    ) -> austringer_providers::identity::ModelIdentity {
        self.publish(SessionEvent::Notification {
            kind: "fallback".into(),
            message: format!(
                "model fallback proposed: {} -> {} ({}); awaiting answer",
                request.from.model, request.to.model, request.reason
            ),
        });
        let chosen = self.mediator.mediate_fallback(request).await;
        self.publish(SessionEvent::Notification {
            kind: "fallback".into(),
            message: format!(
                "model fallback resolved; next requests target {}",
                chosen.model
            ),
        });
        chosen
    }

    /// Highest published event id.
    #[must_use]
    pub fn last_event_id(&self) -> u64 {
        self.with_hub(|hub| hub.last_event_id())
    }

    /// A live-events receiver straight from the hub.
    #[must_use]
    pub fn hub_subscribe(&self) -> broadcast::Receiver<ServerMsg> {
        self.with_hub(|hub| hub.subscribe(None).live)
    }

    /// Full-state snapshot from the hub.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Frame> {
        self.with_hub(|hub| hub.snapshot())
    }

    /// Attach a client. Enforces the per-session cap and refuses
    /// typed on an evicted host (a racing handshake lost to eviction —
    /// the next touch re-creates the session).
    pub fn attach(&self, client_id: &str, after: Option<u64>) -> Result<Attachment, AttachError> {
        if self.is_shutdown() {
            return Err(AttachError::Evicted);
        }
        let mut clients = self.lock_clients();
        if clients.len() >= self.max_clients && !clients.contains(client_id) {
            return Err(AttachError::TooManyClients);
        }
        clients.insert(client_id.to_owned());
        drop(clients);
        let (subscription, last_event_id) =
            self.with_hub(|hub| (hub.subscribe(after), hub.last_event_id()));
        let replay = subscription.replay.clone();
        Ok(Attachment {
            events: subscription,
            control: self.control.subscribe(),
            last_event_id,
            replay,
        })
    }

    /// Detach a client (idempotent).
    pub fn detach(&self, client_id: &str) {
        self.lock_clients().remove(client_id);
    }
}
