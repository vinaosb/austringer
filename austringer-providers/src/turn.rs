//! The resumable turn: mid-stream recovery that preserves turn state.
//!
//! Layering (the retry boundary, from [`crate::retry`]):
//!
//! * **Retry phase** — nothing delivered yet (`accepted == 0`): failures are
//!   transparently retried with jittered exponential backoff. Idempotency
//!   is trivially safe because the consumer has observed nothing.
//! * **Resume phase** — partial output delivered: failures re-issue the SAME
//!   request (stable [`crate::identity::TurnId`]) and the replayed stream is
//!   reconciled by event offset; re-delivered tool events for
//!   already-executed calls are filtered by the [`crate::state::DedupRegistry`].
//!   The model never sees a duplicated tool result and never re-executes a
//!   tool because of a retry.
//!
//! Both phases cap out in typed exhaustion errors. Idle gaps beyond the
//! configured budget cut the stream and feed the same recovery machinery.
//! Cancellation is abandonment: dropping the future (or the turn) drops the
//! underlying stream — nothing drains.

use std::sync::Arc;
use std::time::Duration;

use crate::error::ProviderStreamError;
use crate::event::{CompletedTurn, ProviderEvent};
use crate::identity::{CompletionRequest, TurnId};
use crate::provider::Provider;
use crate::retry::{JitterSource, RetryPolicy};
use crate::state::{AssembledCall, DedupRegistry, TurnSnapshot, TurnState};

// Recovery machinery (attempt lifecycle, budget routing, backoff sleeps)
// lives in `recover.rs` as a sibling impl block.

/// Recovery knobs for a [`ResumableTurn`].
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryConfig {
    /// Pre-first-event retry policy (attempts + jittered backoff).
    pub retry: RetryPolicy,
    /// How many resumes (re-issues with partial state) are allowed.
    pub max_resumes: u32,
    /// Max wait for ANY single event before the stream is considered hung.
    pub idle_timeout: Duration,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            retry: RetryPolicy::default(),
            max_resumes: 4,
            idle_timeout: Duration::from_secs(60),
        }
    }
}

/// A streaming model turn that survives disconnects, stalls, and consumer
/// restarts with exactly-once tool side effects.
pub struct ResumableTurn {
    pub(crate) provider: Arc<dyn Provider>,
    pub(crate) request: CompletionRequest,
    pub(crate) state: TurnState,
    dedup: DedupRegistry,
    pub(crate) config: RecoveryConfig,
    pub(crate) jitter: Box<dyn JitterSource>,
    pub(crate) current: Option<crate::recover::Attempt>,
    pub(crate) attempts_made: u32,
    pub(crate) resumes_made: u32,
    finished: bool,
}

impl ResumableTurn {
    /// Starts a turn against `provider`.
    pub fn start(
        provider: Arc<dyn Provider>,
        request: CompletionRequest,
        config: RecoveryConfig,
        jitter: Box<dyn JitterSource>,
    ) -> Self {
        Self {
            provider,
            request,
            state: TurnState::default(),
            dedup: DedupRegistry::default(),
            config,
            jitter,
            current: None,
            attempts_made: 0,
            resumes_made: 0,
            finished: false,
        }
    }

    /// Rebuilds a turn from a snapshot (consumer restart). The next wire
    /// failure resumes rather than retries, because `accepted > 0`.
    pub fn from_snapshot(
        provider: Arc<dyn Provider>,
        snapshot: TurnSnapshot,
        config: RecoveryConfig,
        jitter: Box<dyn JitterSource>,
    ) -> Self {
        let mut calls = std::collections::BTreeMap::new();
        let mut order = Vec::new();
        for call in &snapshot.calls {
            order.push(call.call_id.clone());
            calls.insert(
                call.call_id.clone(),
                AssembledCall {
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                },
            );
        }
        let mut dedup = DedupRegistry::default();
        for call_id in &snapshot.executed {
            dedup.mark_executed(call_id);
        }
        let finished = snapshot.finish.is_some();
        Self {
            provider,
            request: snapshot.request,
            state: TurnState {
                accepted: snapshot.accepted,
                text: snapshot.text,
                calls,
                order,
                usage: snapshot.usage,
                finish: snapshot.finish,
            },
            dedup,
            config,
            jitter,
            current: None,
            attempts_made: snapshot.attempts_made,
            resumes_made: snapshot.resumes_made,
            finished,
        }
    }

    /// The stable request identity (constant across retries and resumes).
    #[must_use]
    pub fn turn_id(&self) -> &TurnId {
        self.request.turn_id()
    }

    /// The side-effect registry (intended for consumer coordination and
    /// tests; use [`ResumableTurn::mark_executed`] to record).
    #[must_use]
    pub fn dedup(&self) -> &DedupRegistry {
        &self.dedup
    }

    /// Records that `call_id`'s side effect executed. Re-delivered events
    /// for the call are filtered on every subsequent resume.
    pub fn mark_executed(&mut self, call_id: &str) {
        self.dedup.mark_executed(call_id);
    }

    /// A serializable snapshot of the turn (see [`TurnSnapshot`]).
    #[must_use]
    pub fn snapshot(&self) -> TurnSnapshot {
        TurnSnapshot {
            request: self.request.clone(),
            accepted: self.state.accepted,
            text: self.state.text.clone(),
            calls: self
                .state
                .order
                .iter()
                .filter_map(|call_id| {
                    self.state
                        .calls
                        .get(call_id)
                        .map(|assembled| crate::state::SnapshotCall {
                            call_id: call_id.clone(),
                            name: assembled.name.clone(),
                            arguments: assembled.arguments.clone(),
                        })
                })
                .collect(),
            usage: self.state.usage,
            finish: self.state.finish,
            executed: self.dedup.executed().map(str::to_owned).collect(),
            attempts_made: self.attempts_made,
            resumes_made: self.resumes_made,
        }
    }

    /// Yields the next post-recovery event: `None` once the turn finished.
    /// Recovery (retry, resume, idle cut) is driven internally; only typed
    /// terminal errors surface.
    pub async fn next_event(&mut self) -> Option<Result<ProviderEvent, ProviderStreamError>> {
        loop {
            if self.finished {
                return None;
            }
            if self.current.is_none()
                && let Err(err) = self.open().await
            {
                return Some(Err(err));
            }
            let Some(attempt) = self.current.as_mut() else {
                continue;
            };
            let waited =
                tokio::time::timeout(self.config.idle_timeout, attempt.stream.next()).await;
            match waited {
                Err(_elapsed) => {
                    let cause = ProviderStreamError::IdleTimedOut {
                        budget: self.config.idle_timeout,
                    };
                    if let Err(err) = self.abandon_recover(cause).await {
                        return Some(Err(err));
                    }
                }
                Ok(None) => {
                    if self.state.is_finished() {
                        self.finished = true;
                        return None;
                    }
                    let cause = ProviderStreamError::Disconnected {
                        delivered_events: self.state.accepted,
                    };
                    if let Err(err) = self.abandon_recover(cause).await {
                        return Some(Err(err));
                    }
                }
                Ok(Some(Err(err))) => {
                    if !err.is_recoverable() {
                        self.abandon();
                        return Some(Err(err));
                    }
                    if let Err(fatal) = self.abandon_recover(err).await {
                        return Some(Err(fatal));
                    }
                }
                Ok(Some(Ok(event))) => {
                    attempt.cursor += 1;
                    if event.model() != &self.request.identity {
                        let mismatch = ProviderStreamError::IdentityMismatch {
                            requested: self.request.identity.clone(),
                            received: event.model().clone(),
                        };
                        self.abandon();
                        return Some(Err(mismatch));
                    }
                    if attempt.cursor <= attempt.replay_limit {
                        continue;
                    }
                    if self.dedup.filters(&event) {
                        self.state.advance_offset();
                        continue;
                    }
                    let is_finish = matches!(event, ProviderEvent::Finish { .. });
                    self.state.accept(event.clone());
                    if is_finish {
                        self.finished = true;
                    }
                    return Some(Ok(event));
                }
            }
        }
    }

    /// Drives the turn to its finish and assembles the complete-or-error
    /// envelope (todo-5 seam contract (c)).
    pub async fn complete(&mut self) -> Result<CompletedTurn, ProviderStreamError> {
        while let Some(result) = self.next_event().await {
            result?;
        }
        self.state.envelope(self.request.identity.clone())
    }

    /// Drops the in-flight attempt and routes a recoverable failure through
    /// the phase-appropriate budget (retry vs resume), sleeping the jittered
    /// backoff before the next open.
    async fn abandon_recover(
        &mut self,
        cause: ProviderStreamError,
    ) -> Result<(), ProviderStreamError> {
        self.abandon();
        self.recover(cause).await
    }
}
