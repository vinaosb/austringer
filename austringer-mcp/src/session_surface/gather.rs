//! The GATHER fan-in barrier: await N session completions, bounded by a
//! registry capacity and a per-wait deadline (never an unbounded await).

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use crate::session_surface::{SessionId, SurfaceError, lock_or_poisoned};

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
    completed: Arc<Mutex<BTreeSet<SessionId>>>,
    notify: Arc<Notify>,
}

impl GatherRegistry {
    /// A registry bounding every barrier to `capacity` sessions.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            completed: Arc::new(Mutex::new(BTreeSet::new())),
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
    completed: Arc<Mutex<BTreeSet<SessionId>>>,
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
