//! Recovery machinery for [`ResumableTurn`](crate::turn::ResumableTurn):
//! the attempt lifecycle, retry-vs-resume budget routing, and the jittered
//! backoff sleeps. Kept as a sibling impl block so the consumer surface in
//! `turn.rs` stays focused on event delivery.

use crate::error::ProviderStreamError;
use crate::provider::ProviderEventStream;
use crate::turn::ResumableTurn;

/// One in-flight provider attempt plus its replay window.
pub(crate) struct Attempt {
    /// The live typed event stream (dropping it abandons the request).
    pub(crate) stream: ProviderEventStream,
    /// Mapped events consumed from THIS attempt so far.
    pub(crate) cursor: u64,
    /// `accepted` when the attempt opened: identical-replay events at or
    /// below it are suppressed (already in state).
    pub(crate) replay_limit: u64,
}

impl ResumableTurn {
    pub(crate) fn abandon(&mut self) {
        self.current = None;
    }

    /// Drops the in-flight attempt and routes a recoverable failure through
    /// the phase-appropriate budget (retry vs resume), sleeping the jittered
    /// backoff before the next open.
    pub(crate) async fn recover(
        &mut self,
        cause: ProviderStreamError,
    ) -> Result<(), ProviderStreamError> {
        if !cause.is_recoverable() {
            return Err(cause);
        }
        if self.state.accepted == 0 {
            if self.attempts_made >= self.config.retry.max_attempts {
                return Err(ProviderStreamError::RetryExhausted {
                    attempts: self.attempts_made,
                    last: Box::new(cause),
                });
            }
            let backoff = self
                .config
                .retry
                .backoff_for(self.attempts_made, self.jitter.as_mut());
            tokio::time::sleep(backoff).await;
            Ok(())
        } else {
            if self.resumes_made >= self.config.max_resumes {
                return Err(ProviderStreamError::ResumeExhausted {
                    resumes: self.resumes_made,
                    delivered_events: self.state.accepted,
                    last: Box::new(cause),
                });
            }
            let backoff = self
                .config
                .retry
                .backoff_for(self.resumes_made + 1, self.jitter.as_mut());
            tokio::time::sleep(backoff).await;
            Ok(())
        }
    }

    /// Opens the next provider attempt (first, retry, or resume). An
    /// open-time failure routes through the same recovery budgets.
    pub(crate) async fn open(&mut self) -> Result<(), ProviderStreamError> {
        let replay_limit = self.state.accepted;
        self.attempts_made += 1;
        if replay_limit > 0 {
            self.resumes_made += 1;
            tracing::debug!(
                turn_id = self.request.turn_id().as_str(),
                resumes = self.resumes_made,
                replay_limit,
                "resuming turn after interruption"
            );
        }
        match self.provider.stream_completion(&self.request).await {
            Ok(stream) => {
                self.current = Some(Attempt {
                    stream,
                    cursor: 0,
                    replay_limit,
                });
                Ok(())
            }
            Err(err) => self.recover(err).await,
        }
    }
}
