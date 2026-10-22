//! Multi-client session hosting: one [`SessionHost`] per session owns
//! the event hub, the permission mediator, and the client registry.
//! Every attached client reads the same hub, so two clients see
//! byte-identical event streams. The engine driver (real harness-core
//! `AgentLoop` over mediated approval + split-brain execution + shared
//! steering) lives in [`crate::engine`].

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, mpsc};

use harness_core::event::{ContentBlock, MediaBlock, MediaRef, MediaStore};

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
    CostCapReached(harness_core::budget::StopReason),
}

/// Attach failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttachError {
    /// The session's client cap is reached (reconnect-storm bound).
    #[error("session client cap reached")]
    TooManyClients,
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
    cmd_tx: mpsc::Sender<HostCommand>,
    event_sink_tx: mpsc::Sender<SessionEvent>,
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
    /// The interactive plan-mode gate (todo 16); inactive by default.
    plan: crate::plan::PlanGate,
    pub(crate) turn_counter: AtomicU64,
    pub(crate) journal_root: Option<PathBuf>,
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
            cmd_tx,
            event_sink_tx,
            steer_queue: Arc::clone(&steer_queue),
            media_dir: std::sync::OnceLock::new(),
            projection: std::sync::OnceLock::new(),
            skills: std::sync::OnceLock::new(),
            launcher: std::sync::OnceLock::new(),
            cost_ledger: std::sync::OnceLock::new(),
            plan: crate::plan::PlanGate::default(),
            turn_counter: AtomicU64::new(0),
            journal_root,
        });
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

    /// The executor-side event sink (diffs from tool execution).
    #[must_use]
    pub fn event_sink(&self) -> mpsc::Sender<SessionEvent> {
        self.event_sink_tx.clone()
    }

    /// The command sender (transports submit commands through this).
    #[must_use]
    pub fn commands(&self) -> mpsc::Sender<HostCommand> {
        self.cmd_tx.clone()
    }

    /// Intake for transports: Steer resolves IMMEDIATELY (journal +
    /// shared queue — the loop's Steer stage drains it mid-run; the
    /// engine driver is busy inside `run()` and cannot process it),
    /// SubmitMedia resolves its refs against the session media store
    /// (typed refusal on unknown hashes, MediaResult receipts to v2
    /// wires), `/launch` starts a background orchestration launch
    /// (todo 8; the submit itself is the operator authorization),
    /// while plain Submits/SubmitBlocks/Cancel queue to the driver.
    /// Turn-starting submits pass the cost-cap gate first (todo 33):
    /// a stopped session refuses NEW spend typed, naming the cap.
    pub async fn handle_command(
        self: &Arc<Self>,
        command: HostCommand,
    ) -> Result<(), CommandRejected> {
        match &command {
            HostCommand::Steer(text) => {
                self.record_signal(text);
                if let Ok(mut queue) = self.steer_queue.lock() {
                    queue.push_back(text.clone());
                }
            }
            HostCommand::Submit(text) => {
                self.refuse_at_cost_cap()?;
                if text == "/launch" || text.starts_with("/launch ") {
                    let argument = text.strip_prefix("/launch ").unwrap_or_default();
                    return crate::launch::start_launch(self, argument)
                        .map_err(CommandRejected::LaunchRefused);
                }
                let queued = self.route_skills(text)?;
                let _ = self.cmd_tx.send(queued).await;
            }
            HostCommand::SubmitMedia { text, media } => {
                self.refuse_at_cost_cap()?;
                let blocks = self.resolve_media_blocks(text, media)?;
                // Plan M10: needed projections resolve BEFORE the turn
                // queues, so the delivery cache inlines at request-build
                // time; unprojectable media queue Event::System notices.
                if let Some(intake) = self.projection.get() {
                    intake.enqueue_notices(&blocks).await;
                }
                self.queue_blocks(blocks, media);
            }
            HostCommand::SubmitBlocks(_) => {
                self.refuse_at_cost_cap()?;
                let _ = self.cmd_tx.send(command).await;
            }
            HostCommand::Cancel => {
                let _ = self.cmd_tx.send(command).await;
            }
        }
        Ok(())
    }

    /// The cost-cap intake gate (todo 33): a session whose ledger is at
    /// or over its cap refuses turn-starting submits typed, naming the
    /// cap. Steer and Cancel stay open (steering a finishing turn and
    /// cancelling are not new spend); hosts without a ledger pass.
    fn refuse_at_cost_cap(&self) -> Result<(), CommandRejected> {
        if let Some(ledger) = self.cost_ledger.get()
            && let Err(reason) = ledger.preflight()
        {
            return Err(CommandRejected::CostCapReached(reason));
        }
        Ok(())
    }

    /// The skills submit-lane gate (todo 6): every Submit passes the
    /// trust/consent gates — `/skill` commands AND trigger matches.
    /// Refusals are typed and carry the quarantine tier; an activated
    /// skill's context rides the turn as typed blocks while untouched
    /// submits queue in their original form. A host with no runtime
    /// bound (built outside the daemon factory) has no skills surface
    /// at all: `/skill` fails closed, plain text passes untouched.
    fn route_skills(&self, text: &str) -> Result<HostCommand, CommandRejected> {
        let is_skill_command = text.starts_with("/skill ");
        let Some(runtime) = self.skills.get() else {
            return if is_skill_command {
                let name = text
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                Err(CommandRejected::SkillRefused(
                    crate::skills::SkillRefusal::NotBound { name },
                ))
            } else {
                Ok(HostCommand::Submit(text.to_owned()))
            };
        };
        let args = text
            .strip_prefix("/skill ")
            .and_then(|rest| rest.split_once(' ').map(|(_, args)| args))
            .unwrap_or_default();
        match runtime.route_submit(text) {
            crate::skills::SubmitRouting::Pass => Ok(HostCommand::Submit(text.to_owned())),
            crate::skills::SubmitRouting::Refused(refusal) => {
                Err(CommandRejected::SkillRefused(refusal))
            }
            crate::skills::SubmitRouting::Activate(skill) => {
                Ok(HostCommand::SubmitBlocks(vec![ContentBlock::Text {
                    text: crate::skills::SkillsRuntime::render_context(&skill, args),
                }]))
            }
        }
    }

    /// Resolve uploaded refs into typed media blocks: each ref must
    /// exist in the session media store (unknown hash = typed refusal);
    /// the mime + format metadata come from probing the stored bytes.
    fn resolve_media_blocks(
        &self,
        text: &str,
        media: &[MediaRef],
    ) -> Result<Vec<ContentBlock>, CommandRejected> {
        if text.is_empty() && media.is_empty() {
            return Err(CommandRejected::Empty);
        }
        let mut blocks = Vec::new();
        if !text.is_empty() {
            blocks.push(ContentBlock::Text {
                text: text.to_owned(),
            });
        }
        let Some(dir) = self.media_dir.get().cloned() else {
            return Err(match media.first() {
                Some(reference) => CommandRejected::UnknownMedia(reference.hash.to_ref()),
                None => CommandRejected::Empty,
            });
        };
        let store = MediaStore::open(&dir).map_err(|_| {
            CommandRejected::UnknownMedia(
                media
                    .first()
                    .map_or_else(String::new, |reference| reference.hash.to_ref()),
            )
        })?;
        for reference in media {
            let bytes = store
                .load(&reference.hash)
                .map_err(|_| CommandRejected::UnknownMedia(reference.hash.to_ref()))?
                .ok_or_else(|| CommandRejected::UnknownMedia(reference.hash.to_ref()))?;
            let (mime, format_note) = crate::media::probe_mime(&bytes);
            let mut meta = std::collections::BTreeMap::new();
            meta.insert("format".to_owned(), format_note);
            let block = MediaBlock::new(mime, *reference, meta)
                .map_err(|_| CommandRejected::UnknownMedia(reference.hash.to_ref()))?;
            blocks.push(ContentBlock::Media(block));
        }
        Ok(blocks)
    }

    fn queue_blocks(&self, blocks: Vec<ContentBlock>, media: &[MediaRef]) {
        for reference in media {
            self.with_hub(|hub| {
                hub.broadcast(ServerMsg::MediaResult {
                    producer: "daemon-media-intake".into(),
                    artifact: *reference,
                    diff_summary: "media appended to user event".into(),
                })
            });
        }
        let _ = self.cmd_tx.try_send(HostCommand::SubmitBlocks(blocks));
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
        request: harness_providers::identity::FallbackRequest,
    ) -> harness_providers::identity::ModelIdentity {
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

    /// Attach a client. Enforces the per-session cap.
    pub fn attach(&self, client_id: &str, after: Option<u64>) -> Result<Attachment, AttachError> {
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
