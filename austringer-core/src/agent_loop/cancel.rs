//! Cancellation: the cancel token (cancel-marks + grace-window racing)
//! and the typed cancel record naming the high-water sequence.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

use super::operation::Operation;
use crate::event::Sequence;

#[derive(Debug, Default)]
struct CancelState {
    cancelling: bool,
}

/// A sticky cancellation token.
///
/// `cancel()` is idempotent and race-free: every current and future
/// `cancelled()` waiter observes it. Well-behaved stages poll
/// [`CancelToken::is_cancelling`] (or await [`CancelToken::cancelled`])
/// mid-operation and return cooperatively without writing events — that is
/// what makes a cancel land on a clean boundary.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    state: Arc<Mutex<CancelState>>,
    notify: Arc<Notify>,
}

impl CancelToken {
    /// A fresh, uncancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Idempotent.
    pub fn cancel(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.cancelling = true;
        }
        self.notify.notify_waiters();
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelling(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.cancelling)
            .unwrap_or(false)
    }

    /// Resolves once cancellation has been requested.
    ///
    /// Race-free by construction: interest is registered before the flag is
    /// checked, so a `cancel()` that fires between polls is never missed.
    pub async fn cancelled(&self) {
        let mut notified = std::pin::pin!(self.notify.notified());
        notified.as_mut().enable();
        if self.is_cancelling() {
            return;
        }
        notified.await;
    }

    /// Resolves `grace` after a cancellation is observed (immediately
    /// observing an existing one). The loop races this against each
    /// in-flight operation: completion first is a clean cancel, expiry
    /// first is a forced one.
    pub async fn cancelled_then(&self, grace: Duration) {
        self.cancelled().await;
        tokio::time::sleep(grace).await;
    }
}

/// External cancel handle for a running session loop.
#[derive(Debug, Clone, Default)]
pub struct CancelHandle {
    pub(super) token: CancelToken,
}

impl CancelHandle {
    /// Requests cancellation of the session (grace window applies).
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Whether cancellation was already requested.
    #[must_use]
    pub fn is_cancelling(&self) -> bool {
        self.token.is_cancelling()
    }
}

/// The durable fact of a cancel landing: which operation was interrupted,
/// the high-water sequence (the last committed event of completed work —
/// nothing beyond it belongs to a half-written operation), and whether the
/// cancel-grace window expired before a clean boundary was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelRecord {
    /// The operation during which the cancel landed.
    pub operation: Operation,
    /// Last committed sequence before the cancel mark.
    pub high_water: Sequence,
    /// True when the grace window expired and the operation was dropped
    /// mid-flight; false when it reached a clean boundary.
    pub forced: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn token_cancel_is_sticky_and_wakes_waiters() {
        let token = CancelToken::new();
        assert!(!token.is_cancelling());
        token.cancel();
        token.cancel();
        assert!(token.is_cancelling());
        token.cancelled().await;
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_then_elapses_only_after_cancel() {
        let token = CancelToken::new();
        let observer = {
            let token = token.clone();
            tokio::spawn(async move {
                token.cancelled_then(Duration::from_secs(5)).await;
            })
        };
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(!observer.is_finished(), "must not elapse before cancel");
        token.cancel();
        // Virtual-time bounded await: completes deterministically under
        // auto-advance; fails (not hangs) if the grace never elapses.
        let joined = tokio::time::timeout(Duration::from_secs(60), observer).await;
        assert!(joined.is_ok());
    }
}
