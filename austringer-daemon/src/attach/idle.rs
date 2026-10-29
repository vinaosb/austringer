//! Idle/eviction support for [`SessionHost`] (wave 2b): the activity
//! clock, the idle predicate, the eviction shutdown (channels taken,
//! lanes closed), and the engine-task termination observable. The
//! sweep itself lives in [`crate::eviction`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;

use super::{HostCommand, SessionEvent, SessionHost};

impl SessionHost {
    /// The executor-side event sink (diffs from tool execution). On an
    /// evicted host the returned sender is already closed (its
    /// receiver is gone) — sends fail exactly like any closed lane.
    #[must_use]
    pub fn event_sink(&self) -> mpsc::Sender<SessionEvent> {
        match self.event_sink_tx.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
        .unwrap_or_else(|| mpsc::channel(1).0)
    }

    /// The command sender (transports submit commands through this).
    /// On an evicted host the returned sender is already closed.
    #[must_use]
    pub fn commands(&self) -> mpsc::Sender<HostCommand> {
        self.command_sender().unwrap_or_else(|| mpsc::channel(1).0)
    }

    /// The live command sender, or `None` once eviction took the lane.
    pub(crate) fn command_sender(&self) -> Option<mpsc::Sender<HostCommand>> {
        match self.cmd_tx.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Whether any client is attached (a live WS/SSE transport keeps
    /// its host resident — eviction never drops a watched session).
    #[must_use]
    pub fn has_clients(&self) -> bool {
        !self.lock_clients().is_empty()
    }

    /// Whether an engine turn is running (blocks eviction: an active
    /// turn must complete).
    #[must_use]
    pub fn is_turn_in_flight(&self) -> bool {
        self.turn_in_flight.load(Ordering::SeqCst)
    }

    /// Idle means EVICTABLE: no attached clients, no in-flight turn,
    /// and the last turn activity at least `threshold` ago. A host
    /// whose channels were already taken is never re-idle.
    #[must_use]
    pub fn is_idle(&self, threshold: Duration) -> bool {
        if self.has_clients() || self.is_turn_in_flight() || self.is_shutdown() {
            return false;
        }
        let now = unix_millis();
        let age = u128::from(now.saturating_sub(self.last_activity_ms.load(Ordering::SeqCst)));
        age >= threshold.as_millis()
    }

    /// Stamp the activity clock (host construction, each turn start).
    pub(crate) fn mark_activity(&self) {
        self.last_activity_ms.store(unix_millis(), Ordering::SeqCst);
    }

    /// The engine driver's loop ended (the observable that eviction
    /// actually freed the engine and its transcript mirror).
    #[must_use]
    pub fn engine_task_finished(&self) -> bool {
        self.engine_task_done.load(Ordering::SeqCst)
    }

    pub(crate) fn mark_engine_task_finished(&self) {
        self.engine_task_done.store(true, Ordering::SeqCst);
    }

    /// Whether eviction took this host's channels.
    pub(crate) fn is_shutdown(&self) -> bool {
        self.evicted.load(Ordering::SeqCst)
    }

    /// Eviction shutdown: take and drop the host's own channel
    /// senders. The engine driver's `recv()` ends once transient
    /// clones release, its task finishes (freeing the `AgentLoop` and
    /// the stage-held clones — including the executor's sink clone,
    /// which then ends the sink pump). Intake and attach refuse typed
    /// from here on; the next touch of the session id re-creates the
    /// host.
    pub(crate) fn shutdown_channels(&self) {
        self.evicted.store(true, Ordering::SeqCst);
        let cmd = match self.cmd_tx.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        let sink = match self.event_sink_tx.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        drop(cmd);
        drop(sink);
    }
}

/// RAII in-flight marker: set on turn start, cleared on scope exit —
/// including a panicking unwind — so a crashed engine never wedges a
/// host un-evictable.
pub(crate) struct InFlightTurn<'a> {
    flag: &'a AtomicBool,
}

impl<'a> InFlightTurn<'a> {
    /// Mark `host` as running a turn for the guard's lifetime.
    pub(crate) fn mark(host: &'a SessionHost) -> Self {
        host.turn_in_flight.store(true, Ordering::SeqCst);
        Self {
            flag: &host.turn_in_flight,
        }
    }
}

impl Drop for InFlightTurn<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            since.as_millis().min(u128::from(u64::MAX)) as u64
        })
}
