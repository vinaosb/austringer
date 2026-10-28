//! Idle-session eviction (wave 2b): the memory-footprint counterpart
//! to the retention janitor. An abandoned session host pins its engine
//! driver — and with it the `AgentLoop`'s unbounded events mirror —
//! for the daemon's lifetime; the sweep drops the map entry after
//! closing the host's channels, so the engine task observes a closed
//! command lane, finishes, and frees the loop + transcript. The next
//! touch of the id re-creates the host transparently
//! ([`AgentLoop::open`](austringer_core::agent_loop::AgentLoop::open)
//! resumes from the session dir; the cost ledger survives in the
//! daemon-level map). Hosts with attached clients or an in-flight
//! turn are never evicted.

use std::sync::Arc;
use std::time::Duration;

use crate::server::DaemonState;

/// One sweep per hour: eviction is background hygiene, never a
/// latency path (the retention janitor's cadence).
const SWEEP_INTERVAL: Duration = Duration::from_secs(3600);

/// Spawn the eviction loop: one sweep at startup, then one every
/// [`SWEEP_INTERVAL`], at the configured idle threshold. A
/// `session_idle_eviction` of [`Duration::ZERO`] keeps eviction off
/// entirely — no task is spawned.
pub fn spawn(state: Arc<DaemonState>) {
    if state.config.session_idle_eviction.is_zero() {
        return;
    }
    tokio::spawn(async move {
        let idle = state.config.session_idle_eviction;
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let evicted = run_once(&state, idle);
            if !evicted.is_empty() {
                tracing::info!(count = evicted.len(), sessions = ?evicted, "idle-session eviction sweep");
            }
        }
    });
}

/// One eviction sweep: hosts with attached clients, in-flight turns,
/// or recent activity stay; the rest close their channels and leave
/// the map. Returns the evicted session ids. `Duration::ZERO` is the
/// disabled posture (nothing evicts). Exposed so tests (and future
/// admin routes) can run a sweep without waiting on the interval.
#[must_use]
pub fn run_once(state: &DaemonState, idle: Duration) -> Vec<String> {
    if idle.is_zero() {
        return Vec::new();
    }
    let mut evicted = Vec::new();
    let mut hosts = match state.hosts.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    hosts.retain(|id, host| {
        if host.is_idle(idle) {
            host.shutdown_channels();
            evicted.push(id.clone());
            false
        } else {
            true
        }
    });
    evicted
}
