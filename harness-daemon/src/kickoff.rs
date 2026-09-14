//! Session kickoff seam: what a fired automation starts, who executes
//! the kickoff, and the durable fire/skip activities the runner
//! journals (a replayed fire never re-kicks).

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::activity::{ActivityError, ActivityExecutor};
use crate::attach::HostCommand;
use crate::automations::NetworkPolicy;
use crate::server::DaemonState;

/// Durable fire activity name.
pub const FIRE_ACTIVITY: &str = "scheduled.fire";
/// Durable cap-skip activity name.
pub const SKIP_ACTIVITY: &str = "scheduled.skip";

/// What a fired automation starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KickOff {
    /// Owning automation.
    pub automation_id: String,
    /// Deterministic session id (`auto-{job}-{minute}`).
    pub session_id: String,
    /// Pack under run.
    pub pack: String,
    /// Prompt handed to the session.
    pub prompt: String,
    /// Per-run turn budget.
    pub budget_turns: Option<u32>,
    /// Effective (intersected) network policy.
    pub network: NetworkPolicy,
}

/// Kickoff failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KickError {
    /// The session's command queue is full (retry later).
    #[error("session command queue full")]
    QueueFull,
    /// Kickoff failed for another reason.
    #[error("kick off: {0}")]
    Spawn(String),
}

/// Executes a kickoff: creates/binds the session and submits the run.
pub trait RunKicker: Send + Sync {
    /// Kick off `run`.
    fn kick_off(&self, run: &KickOff) -> Result<(), KickError>;

    /// Re-acquire the session host so the completion watcher can
    /// observe it (None = not watchable; tests/embedders).
    fn host_for(&self, _session_id: &str) -> Option<Arc<crate::attach::SessionHost>> {
        None
    }
}

/// Production kicker over a live [`DaemonState`]: scheduled sessions
/// are regular URL-addressable sessions with a per-run turn budget.
pub struct DaemonKicker {
    state: Arc<DaemonState>,
}

impl DaemonKicker {
    /// Kicker over daemon state.
    #[must_use]
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
    }
}

impl RunKicker for DaemonKicker {
    fn kick_off(&self, run: &KickOff) -> Result<(), KickError> {
        let host = self
            .state
            .session_scheduled(&run.session_id, run.budget_turns);
        host.commands()
            .try_send(HostCommand::Submit(run.prompt.clone()))
            .map_err(|e| match e {
                tokio::sync::mpsc::error::TrySendError::Full(_) => KickError::QueueFull,
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    KickError::Spawn("session driver gone".into())
                }
            })
    }

    fn host_for(&self, session_id: &str) -> Option<Arc<crate::attach::SessionHost>> {
        Some(self.state.session(session_id))
    }
}

/// The journaled fire activity input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FireInput {
    /// The kickoff to perform.
    pub(crate) kick: KickOff,
    /// The cron minute that fired.
    pub(crate) minute: u64,
}

/// Inner executor behind the runner's journaling executor: performs
/// the actual kickoff, or records a cap-skip decision.
pub(crate) struct KickoffExecutor {
    kicker: Arc<dyn RunKicker>,
}

impl KickoffExecutor {
    /// Executor delegating to `kicker`.
    pub(crate) fn new(kicker: Arc<dyn RunKicker>) -> Self {
        Self { kicker }
    }
}

impl ActivityExecutor for KickoffExecutor {
    fn execute(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, ActivityError> {
        if name == SKIP_ACTIVITY {
            // Cap decisions journal durably: a replayed skip must not
            // be reconsidered either.
            let skipped = input.get("reason").cloned().unwrap_or_default();
            return Ok(serde_json::json!({ "skipped": skipped }));
        }
        if name != FIRE_ACTIVITY {
            return Err(ActivityError {
                message: format!("unknown activity {name}"),
                retryable: false,
            });
        }
        let fire: FireInput = serde_json::from_value(input.clone()).map_err(|e| ActivityError {
            message: format!("fire input: {e}"),
            retryable: false,
        })?;
        self.kicker
            .kick_off(&fire.kick)
            .map_err(|e| ActivityError {
                message: e.to_string(),
                retryable: matches!(e, KickError::QueueFull),
            })?;
        Ok(serde_json::json!({ "session_id": fire.kick.session_id }))
    }
}
