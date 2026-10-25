//! Per-language slot lifecycle: lazy spawn, crash accounting, deterministic
//! backoff, and degradation. One slot per language; lifecycle transitions
//! serialize on a tokio mutex while state itself lives behind a std mutex
//! so the (sync) death hook can mark crashes without blocking a runtime.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::Sender;

use crate::config::{Backoff, Language, LspConfig};
use crate::error::DegradeReason;
use crate::native::{PositionEncoding, ServerHandle};
use crate::router::RouterEvent;
use crate::{LspError, lock};

/// What `ensure_ready` hands back.
pub(crate) enum Ensure {
    /// A live, initialized server.
    Ready(Arc<ServerHandle>, PositionEncoding),
    /// The language is off; callers degrade to lexical search.
    Degraded(DegradeReason),
}

enum Stage {
    /// Never spawned.
    Idle,
    /// Initialized and believed alive.
    Ready(Arc<ServerHandle>, PositionEncoding),
    /// Died (transport/hang); eligible for restart.
    Crashed,
    /// Terminal for this router instance.
    Degraded(DegradeReason),
}

/// The restart state machine for one language. Owns the per-server
/// document tracker: a (re)spawned server starts with NO open documents,
/// so didOpen re-fires after every restart.
pub(crate) struct Slot {
    language: Language,
    state: Mutex<Stage>,
    lifecycle: tokio::sync::Mutex<()>,
    crashes: AtomicU32,
    pub(crate) docs: crate::docs::DocTracker,
}

impl Slot {
    pub(crate) fn new(language: Language) -> Self {
        Self {
            language,
            state: Mutex::new(Stage::Idle),
            lifecycle: tokio::sync::Mutex::new(()),
            crashes: AtomicU32::new(0),
            docs: crate::docs::DocTracker::new(),
        }
    }

    /// The live handle if the slot is Ready, for shutdown and the RSS
    /// guard. Reports `None` for dead/ready handles.
    pub(crate) fn ready_handle(&self) -> Option<Arc<ServerHandle>> {
        let state = lock(&self.state);
        match &*state {
            Stage::Ready(handle, _) if !handle.is_dead() => Some(Arc::clone(handle)),
            _ => None,
        }
    }

    /// Installs a Ready state directly (the `attach_server` path, which
    /// ran the handshake itself). Crash accounting is untouched; the
    /// attached server starts with no open documents.
    pub(crate) fn force_ready(&self, handle: Arc<ServerHandle>, encoding: PositionEncoding) {
        *lock(&self.state) = Stage::Ready(handle, encoding);
        self.docs.clear();
    }

    /// Marks the slot crashed (idempotent; only a Ready slot transitions),
    /// bumps the crash counter, and emits [`RouterEvent::ServerCrashed`].
    pub(crate) fn mark_crashed(&self, events: &Sender<RouterEvent>) {
        let transitioned = {
            let mut state = lock(&self.state);
            match &*state {
                Stage::Ready(handle, _) if handle.is_dead() => {
                    *state = Stage::Crashed;
                    true
                }
                _ => false,
            }
        };
        if transitioned {
            self.crashes.fetch_add(1, Ordering::AcqRel);
            let _ = events.try_send(RouterEvent::ServerCrashed {
                language: self.language,
            });
        }
    }

    /// Resets to Idle with a fresh crash budget (used by explicit
    /// `shutdown()`, which ends server lifetimes deliberately).
    pub(crate) fn reset(&self) {
        *lock(&self.state) = Stage::Idle;
        self.crashes.store(0, Ordering::Release);
    }

    /// Brings the slot to Ready — spawning (lazily, once) through `spawn`,
    /// restarting with deterministic backoff after crashes, and degrading
    /// once the crash budget is spent.
    pub(crate) async fn ensure_ready<S, F>(
        &self,
        config: &LspConfig,
        events: &Sender<RouterEvent>,
        spawn: S,
    ) -> Result<Ensure, LspError>
    where
        S: FnOnce() -> F,
        F: std::future::Future<Output = Result<(Arc<ServerHandle>, PositionEncoding), LspError>>,
    {
        let _lifecycle = self.lifecycle.lock().await;
        if let Some(ensure) = self.fast_path() {
            return Ok(ensure);
        }
        let crashes = self.crashes.load(Ordering::Acquire);
        if crashes > config.max_restarts {
            let reason = DegradeReason::RestartBudgetExhausted { crashes };
            *lock(&self.state) = Stage::Degraded(reason);
            let _ = events.try_send(RouterEvent::Degraded {
                language: self.language,
                reason,
            });
            return Ok(Ensure::Degraded(reason));
        }
        if crashes >= 1 {
            Self::sleep_backoff(&config.backoff, crashes, self.language, events).await;
        }
        match spawn().await {
            Ok((handle, encoding)) => {
                let restarted = crashes >= 1;
                *lock(&self.state) = Stage::Ready(Arc::clone(&handle), encoding);
                // A new server process has never seen our documents.
                self.docs.clear();
                let event = if restarted {
                    RouterEvent::ServerRestarted {
                        language: self.language,
                    }
                } else {
                    RouterEvent::ServerSpawned {
                        language: self.language,
                    }
                };
                let _ = events.try_send(event);
                Ok(Ensure::Ready(handle, encoding))
            }
            // Spawn/init failures surface their typed cause to THIS
            // caller; the crash counter is bumped, so the NEXT call
            // walks backoff -> retry -> degrade.
            Err(err) => {
                self.crashes.fetch_add(1, Ordering::AcqRel);
                Err(err)
            }
        }
    }

    fn fast_path(&self) -> Option<Ensure> {
        let state = lock(&self.state);
        match &*state {
            Stage::Ready(handle, encoding) if !handle.is_dead() => {
                Some(Ensure::Ready(Arc::clone(handle), *encoding))
            }
            Stage::Degraded(reason) => Some(Ensure::Degraded(*reason)),
            _ => None,
        }
    }

    async fn sleep_backoff(
        backoff: &Backoff,
        crashes: u32,
        language: Language,
        events: &Sender<RouterEvent>,
    ) {
        let wait: Duration = backoff.wait_for(crashes);
        let _ = events.try_send(RouterEvent::RestartScheduled {
            language,
            attempt: crashes,
            wait,
        });
        tokio::time::sleep(wait).await;
    }
}
