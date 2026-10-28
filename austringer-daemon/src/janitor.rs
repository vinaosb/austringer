//! The sessions-retention janitor: the daemon-side caller that puts
//! the (wave-1) dormant `SessionStore::prune` to work. One background
//! task per daemon — spawned ONLY when a retention policy is
//! configured — runs the tiers on a fixed interval, logs the
//! itemized report, removes the engine-side orphan directories of
//! GC'd sessions (`{data}/sessions/{id}/` and `{data}/journal/{id}/`;
//! the store removes its own `{data}/{id}/`), and optionally vacuums
//! the store after a deleting pass.
//!
//! Crash-safety first: ONLY ids the prune report named in
//! `gc_removed` ever drive a directory deletion — no speculative
//! orphan sweeping — and a session that is live again (host present,
//! or its row re-created after the prune) is never touched.
//!
//! Shutdown hygiene lives here too: [`remove_instance_files`] clears
//! this daemon's readiness files when it stops, but only while they
//! still point at this instance.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use austringer_sessions::{PruneReport, RetentionPolicy, SessionStore};

use crate::server::DaemonState;

/// One pass per hour: the janitor is background hygiene, never a
/// latency path.
const PASS_INTERVAL: Duration = Duration::from_secs(3600);

/// What one janitor pass did.
#[derive(Debug, Clone, PartialEq)]
pub struct JanitorPass {
    /// The itemized prune report (already logged by the pass).
    pub report: PruneReport,
    /// Whether the opt-in post-prune `VACUUM` ran.
    pub vacuumed: bool,
}

/// Janitor failures. A failed pass never stops the loop — the next
/// interval retries.
#[derive(Debug, thiserror::Error)]
pub enum JanitorError {
    /// Opening the sessions store failed.
    #[error("open sessions store: {0}")]
    Open(String),
    /// The prune pass failed.
    #[error("prune: {0}")]
    Prune(String),
    /// Listing post-prune sessions failed (cleanup stays fail-closed).
    #[error("list sessions: {0}")]
    List(String),
}

/// Spawn the janitor loop: one pass at startup, then one every
/// [`PASS_INTERVAL`]. The all-`None` default policy keeps the janitor
/// off entirely — no task is spawned, nothing changes.
pub fn spawn(state: Arc<DaemonState>) {
    if state.config.retention == RetentionPolicy::default() {
        return;
    }
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(PASS_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(error) = run_once(&state) {
                tracing::warn!("retention janitor pass failed: {error}");
            }
        }
    });
}

/// One janitor pass over the daemon's sessions store: prune, log,
/// orphan cleanup, opt-in vacuum. Exposed so tests (and future admin
/// routes) can run a pass without waiting on the interval.
///
/// # Errors
/// Typed [`JanitorError`]. Per-directory cleanup failures are logged
/// and skipped, never fatal.
pub fn run_once(state: &DaemonState) -> Result<JanitorPass, JanitorError> {
    let config = &state.config;
    let store =
        SessionStore::open(&config.data_dir).map_err(|e| JanitorError::Open(e.to_string()))?;
    let report = store
        .prune(&config.retention)
        .map_err(|e| JanitorError::Prune(e.to_string()))?;
    tracing::info!(
        archived = report.archived.len(),
        gc_removed = report.gc_removed.len(),
        pinned_skipped = report.pinned_skipped.len(),
        skipped_encrypted = report.skipped_encrypted.len(),
        "retention janitor pass"
    );
    // Fail-closed cleanup: ids the store still knows (or that are
    // live as hosts) are never removed, whatever the report says.
    let known: HashSet<String> = store
        .list()
        .map_err(|e| JanitorError::List(e.to_string()))?
        .into_iter()
        .map(|meta| meta.id)
        .collect();
    for id in &report.gc_removed {
        if state.try_session(id).is_some() {
            tracing::warn!(session = %id, "gc'd session has a live host; dirs preserved");
            continue;
        }
        if known.contains(id) {
            tracing::warn!(session = %id, "gc'd session is back in the store; dirs preserved");
            continue;
        }
        remove_dir_best_effort(&config.data_dir.join("sessions").join(id));
        remove_dir_best_effort(&config.data_dir.join("journal").join(id));
        // The cost ledger outlives host eviction but not the session
        // itself: a GC'd session's spend entry goes with its dirs.
        match state.ledgers.lock() {
            Ok(mut ledgers) => {
                ledgers.remove(id);
            }
            Err(poisoned) => {
                poisoned.into_inner().remove(id);
            }
        }
    }
    let deleted = !report.archived.is_empty() || !report.gc_removed.is_empty();
    let vacuumed = config.retention_vacuum
        && deleted
        && match store.vacuum() {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!("retention janitor vacuum failed: {error}");
                false
            }
        };
    Ok(JanitorPass { report, vacuumed })
}

/// Remove one orphan directory best-effort: absent is fine, other
/// failures are logged, the pass never panics.
fn remove_dir_best_effort(path: &Path) {
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "janitor orphan cleanup failed")
        }
    }
}

/// Shutdown hygiene: remove this daemon's readiness files
/// (`daemon-url`, `daemon-token`) from the data dir — but only while
/// their contents still point at THIS instance. A reload successor
/// (or any other daemon sharing the dir) that already rewrote them
/// owns them now; absent files are a no-op.
pub fn remove_instance_files(data_dir: &Path, url: &str, token: &str) {
    for (name, expected) in [("daemon-url", url), ("daemon-token", token)] {
        let path = data_dir.join(name);
        match std::fs::read_to_string(&path) {
            Ok(content) if content == expected => {
                if let Err(error) = std::fs::remove_file(&path) {
                    tracing::warn!(path = %path.display(), %error, "shutdown file cleanup failed");
                }
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
}
