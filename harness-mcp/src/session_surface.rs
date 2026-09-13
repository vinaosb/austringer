//! The server-mode session surface (todo 29, Devin pattern): the typed
//! `SessionControl` seam the harness-as-MCP-server tools sit on, cost
//! limits, and the GATHER fan-in barrier (await N session completions,
//! bounded).
//!
//! Seam justification: harness-sessions is a sibling storage crate; a
//! hard dependency would couple the MCP surface to storage internals
//! (SQLite, encryption) and drag them into every embedding. The daemon
//! implements this trait over T24's store; this crate ships the
//! scripted in-test implementation for protocol tests.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use harness_core::budget::MicroUsd;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

/// Opaque session identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Session lifecycle status (server-mode view).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Created, no interaction yet.
    Idle,
    /// An interaction is in flight.
    InProgress,
    /// Terminal success.
    Completed,
    /// Terminal failure.
    Failed,
    /// Cancelled by the operator.
    Cancelled,
}

/// One session's summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Opaque identity.
    pub id: SessionId,
    /// Human title, if any.
    pub title: Option<String>,
    /// Lifecycle status.
    pub status: SessionStatus,
}

/// `session_create` request.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateRequest {
    /// Optional human title.
    pub title: Option<String>,
}

/// `session_interact` request: one prompt against one session under an
/// optional cost ceiling.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractRequest {
    /// Target session.
    pub session: SessionId,
    /// The prompt text.
    pub prompt: String,
    /// Hard cost ceiling for this interaction (micro-USD); `None` = no
    /// explicit per-call limit (session/task budgets still apply in the
    /// wired engine).
    pub cost_limit: Option<MicroUsd>,
}

/// Cost-limit outcome for one interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostLimitOutcome {
    /// The interaction ran within its ceiling.
    WithinLimit,
    /// No explicit ceiling was supplied.
    NoLimit,
}

/// `session_interact` outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionOutcome {
    /// The engine's reply text.
    pub reply: String,
    /// Cost-ceiling bookkeeping.
    pub cost: CostLimitOutcome,
}

/// One bounded event chunk (`session_events`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventChunk {
    /// Monotonic ordinal within the session.
    pub ordinal: u64,
    /// Event kind (user/assistant/tool/system…).
    pub kind: String,
    /// Rendered text (already bounded by the seam impl).
    pub text: String,
}

/// Why a session-surface call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SurfaceError {
    /// The session id is unknown to the backing store.
    #[error("unknown session {0}")]
    UnknownSession(SessionId),
    /// The interaction would exceed its explicit cost ceiling; nothing
    /// ran.
    #[error("cost limit exceeded: requested {} µUSD > limit {} µUSD", requested.0, limit.0)]
    CostLimitExceeded {
        requested: MicroUsd,
        limit: MicroUsd,
    },
    /// A gather was asked to await more sessions than its capacity
    /// bound allows.
    #[error("gather unbounded: {requested} sessions > capacity {capacity}")]
    GatherUnbounded { requested: usize, capacity: usize },
    /// The backing engine failed.
    #[error("session engine failure: {0}")]
    Engine(String),
}

/// The seam the daemon implements over the T24 sessions store.
#[async_trait]
pub trait SessionControl: Send + Sync {
    /// Creates a session.
    async fn create(&self, request: CreateRequest) -> Result<SessionSummary, SurfaceError>;
    /// Searches sessions by text.
    async fn search(&self, query: &str) -> Result<Vec<SessionSummary>, SurfaceError>;
    /// Sends one prompt under an optional cost ceiling.
    async fn interact(&self, request: InteractRequest) -> Result<InteractionOutcome, SurfaceError>;
    /// Reads bounded event chunks strictly after `since`.
    async fn events(
        &self,
        session: &SessionId,
        since: u64,
    ) -> Result<Vec<EventChunk>, SurfaceError>;
}

/// The scripted in-test implementation (deterministic; zero-limit = the
/// cost ceiling always trips, which keeps the typed path exercised).
#[derive(Debug, Default)]
pub struct ScriptedSessionControl {
    inner: Mutex<ScriptedInner>,
}

#[derive(Debug, Default)]
struct ScriptedInner {
    next_id: u32,
    sessions: BTreeMap<String, SessionSummary>,
}

#[async_trait]
impl SessionControl for ScriptedSessionControl {
    async fn create(&self, request: CreateRequest) -> Result<SessionSummary, SurfaceError> {
        let mut inner = lock_or_poisoned(&self.inner);
        inner.next_id += 1;
        let id = format!("scripted-{}", inner.next_id);
        let summary = SessionSummary {
            id: SessionId(id.clone()),
            title: request.title,
            status: SessionStatus::Idle,
        };
        inner.sessions.insert(id, summary.clone());
        Ok(summary)
    }

    async fn search(&self, query: &str) -> Result<Vec<SessionSummary>, SurfaceError> {
        let inner = lock_or_poisoned(&self.inner);
        Ok(inner
            .sessions
            .values()
            .filter(|summary| {
                summary
                    .title
                    .as_deref()
                    .is_some_and(|title| title.contains(query))
            })
            .cloned()
            .collect())
    }

    async fn interact(&self, request: InteractRequest) -> Result<InteractionOutcome, SurfaceError> {
        {
            let inner = lock_or_poisoned(&self.inner);
            if !inner.sessions.contains_key(&request.session.0) {
                return Err(SurfaceError::UnknownSession(request.session));
            }
        }
        if let Some(limit) = request.cost_limit
            && limit == MicroUsd(0)
        {
            return Err(SurfaceError::CostLimitExceeded {
                requested: MicroUsd(1),
                limit,
            });
        }
        Ok(InteractionOutcome {
            reply: "scripted reply".to_owned(),
            cost: request
                .cost_limit
                .map(|_| CostLimitOutcome::WithinLimit)
                .unwrap_or(CostLimitOutcome::NoLimit),
        })
    }

    async fn events(
        &self,
        session: &SessionId,
        since: u64,
    ) -> Result<Vec<EventChunk>, SurfaceError> {
        let inner = lock_or_poisoned(&self.inner);
        if !inner.sessions.contains_key(&session.0) {
            return Err(SurfaceError::UnknownSession(session.clone()));
        }
        Ok(vec![EventChunk {
            ordinal: since + 1,
            kind: "assistant".to_owned(),
            text: "scripted reply".to_owned(),
        }])
    }
}

fn lock_or_poisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// The GATHER fan-in barrier outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatherOutcome {
    /// Every awaited session completed.
    AllCompleted { completed: usize, timed_out: usize },
    /// The deadline hit with some sessions still outstanding.
    Partial { completed: usize, timed_out: usize },
}

/// Registry of session-completion signals; barriers await slices of it.
pub struct GatherRegistry {
    capacity: usize,
    completed: Arc<Mutex<std::collections::BTreeSet<SessionId>>>,
    notify: Arc<Notify>,
}

impl GatherRegistry {
    /// A registry bounding every barrier to `capacity` sessions.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            completed: Arc::new(Mutex::new(std::collections::BTreeSet::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Records one session completion (wakes waiting barriers).
    pub fn complete(&self, id: SessionId) {
        lock_or_poisoned(&self.completed).insert(id);
        self.notify.notify_waiters();
    }

    /// Starts a barrier over `sessions`; typed rejection when the set
    /// exceeds the capacity bound.
    pub fn try_barrier(&self, sessions: Vec<SessionId>) -> Result<GatherBarrier, SurfaceError> {
        if sessions.len() > self.capacity {
            return Err(SurfaceError::GatherUnbounded {
                requested: sessions.len(),
                capacity: self.capacity,
            });
        }
        Ok(GatherBarrier {
            sessions,
            completed: Arc::clone(&self.completed),
            notify: Arc::clone(&self.notify),
        })
    }

    /// Barrier that resolves as soon as every session completed.
    #[must_use]
    pub fn barrier(&self, sessions: Vec<SessionId>) -> GatherBarrier {
        self.try_barrier(sessions).unwrap_or(GatherBarrier {
            sessions: Vec::new(),
            completed: Arc::clone(&self.completed),
            notify: Arc::clone(&self.notify),
        })
    }
}

/// One bounded wait over N session completions.
pub struct GatherBarrier {
    sessions: Vec<SessionId>,
    completed: Arc<Mutex<std::collections::BTreeSet<SessionId>>>,
    notify: Arc<Notify>,
}

impl GatherBarrier {
    /// Waits until every session completed or `deadline` elapses
    /// (bounded; never an unbounded await).
    pub async fn wait(&self, deadline: Duration) -> GatherOutcome {
        let deadline_at = Instant::now() + deadline;
        loop {
            let done = self.count_completed();
            if done == self.sessions.len() {
                return GatherOutcome::AllCompleted {
                    completed: done,
                    timed_out: 0,
                };
            }
            let remaining = deadline_at.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return GatherOutcome::Partial {
                    completed: done,
                    timed_out: self.sessions.len() - done,
                };
            }
            let notified = self.notify.notified();
            tokio::pin!(notified);
            tokio::select! {
                () = &mut notified => {}
                () = tokio::time::sleep(remaining) => {
                    let done = self.count_completed();
                    return GatherOutcome::Partial {
                        completed: done,
                        timed_out: self.sessions.len() - done,
                    };
                }
            }
        }
    }

    fn count_completed(&self) -> usize {
        let completed = lock_or_poisoned(&self.completed);
        self.sessions
            .iter()
            .filter(|id| completed.contains(*id))
            .count()
    }
}
