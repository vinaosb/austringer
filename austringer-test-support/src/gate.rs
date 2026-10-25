//! [`ResponseGate`]: a Condvar-style async gate for race and cancellation
//! tests. Producers hold consumers at a deterministic rendezvous point instead
//! of timing-sensitive sleeps.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::Notify;

#[derive(Debug, Default)]
struct GateState {
    released: bool,
    cancelled: bool,
}

/// Error returned when a gate is cancelled before being released.
#[derive(Debug, thiserror::Error, PartialEq)]
#[error("gate closed before release")]
pub struct GateClosed;

/// Error returned by [`ResponseGate::wait_for`].
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GateWaitError {
    /// The bounded wait elapsed without a release.
    #[error("gate wait timed out")]
    Timeout,
    /// The gate was cancelled before being released.
    #[error(transparent)]
    Closed(#[from] GateClosed),
}

/// An async condition gate: `wait()` blocks until `release()` or `cancel()`.
///
/// Unlike a raw `Notify`, the gate is stateful and race-free: waiters that
/// register before OR after a `release()` observe it, `cancel()` is sticky for
/// future waiters, and a release that already happened always wins over a later
/// cancel. Use it to sequence producer/consumer tasks deterministically.
///
/// ```
/// # async fn demo() {
/// use austringer_test_support::gate::ResponseGate;
/// let gate = ResponseGate::new();
/// gate.release();
/// assert_eq!(gate.wait().await, Ok(()));
/// # }
/// ```
#[derive(Debug, Default)]
pub struct ResponseGate {
    state: Mutex<GateState>,
    notify: Notify,
}

impl ResponseGate {
    /// Creates a closed (unreleased, uncancelled) gate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the gate: every current and future waiter proceeds.
    pub fn release(&self) {
        self.lock().released = true;
        self.notify.notify_waiters();
    }

    /// Cancels the gate: every waiter that has not observed a release fails
    /// with [`GateClosed`], and so does every future `wait`.
    pub fn cancel(&self) {
        self.lock().cancelled = true;
        self.notify.notify_waiters();
    }

    /// Whether `release()` has been called (regardless of a later cancel).
    pub fn is_released(&self) -> bool {
        self.lock().released
    }

    /// Waits until the gate is released. Returns [`GateClosed`] if the gate was
    /// cancelled first. Race-free by construction: interest is registered with
    /// the `Notify` *before* the state is checked, so no wakeup can be lost.
    pub async fn wait(&self) -> Result<(), GateClosed> {
        let mut notified = std::pin::pin!(self.notify.notified());
        notified.as_mut().enable();
        {
            let state = self.lock();
            if state.released {
                return Ok(());
            }
            if state.cancelled {
                return Err(GateClosed);
            }
        }
        notified.await;
        let state = self.lock();
        if state.released {
            Ok(())
        } else {
            Err(GateClosed)
        }
    }

    /// Like [`ResponseGate::wait`], but gives up after `timeout`.
    pub async fn wait_for(&self, timeout: Duration) -> Result<(), GateWaitError> {
        match tokio::time::timeout(timeout, self.wait()).await {
            Ok(result) => result.map_err(GateWaitError::Closed),
            Err(_) => Err(GateWaitError::Timeout),
        }
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        // No code panics while holding this lock (critical sections are flag
        // writes), so poisoning cannot occur; recover defensively regardless.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
