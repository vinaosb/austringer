//! Typed stream errors for the provider layer.

use std::time::Duration;

use crate::identity::ModelIdentity;

/// Why the provider rejected a request outright (never retried).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    /// 200 OK but zero choices.
    #[error("no choices in response")]
    NoChoices,
    /// Truncated by the output token limit.
    #[error("output token limit reached")]
    OutputLimit,
    /// Rejected for exceeding the context window.
    #[error("context length exceeded")]
    ContextLimit,
}

/// Every failure mode of a provider stream, typed. Stringly errors stop at
/// the adapter boundary.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProviderStreamError {
    /// HTTP-level failure (status, provider error code, message).
    #[error("provider http error: status {status}, code {code}: {message}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Provider-specific error code.
        code: String,
        /// Human-readable message.
        message: String,
    },
    /// The provider rejected the request; retrying cannot help.
    #[error("provider rejected request: {0}")]
    Rejected(Rejection),
    /// The stream ended before a finish reason arrived (dropped connection,
    /// truncated body). Recovery territory.
    #[error("stream disconnected after {delivered_events} delivered events")]
    Disconnected {
        /// Events already delivered to the consumer before the drop.
        delivered_events: u64,
    },
    /// No event arrived within the configured idle budget.
    #[error("no provider event within idle budget {budget:?}")]
    IdleTimedOut {
        /// The configured budget that elapsed.
        budget: Duration,
    },
    /// An event carried an identity other than the requested one. Fatal:
    /// the identity contract tolerates no silent substitution.
    #[error("identity mismatch: requested {requested:?} but event came from {received:?}")]
    IdentityMismatch {
        /// The identity the request targeted.
        requested: ModelIdentity,
        /// The identity the event carried.
        received: ModelIdentity,
    },
    /// Assembled tool-call arguments are not valid JSON.
    #[error("tool call {call_id} produced invalid JSON arguments")]
    InvalidArguments {
        /// The call whose arguments failed to parse.
        call_id: String,
    },
    /// A tool call streamed deltas but never a `ToolCallStart`, so its name
    /// is unknowable.
    #[error("tool call {call_id} never received a start event")]
    UnnamedToolCall {
        /// The orphaned call id.
        call_id: String,
    },
    /// The provider emitted an in-stream error event.
    #[error("provider stream error ({code}): {message}")]
    StreamError {
        /// Provider error code.
        code: String,
        /// Human-readable message.
        message: String,
    },
    /// The retry budget ran out before any event reached the consumer.
    /// (Retry is idempotency-safe only in that phase: nothing was emitted.)
    #[error("retry budget exhausted after {attempts} attempts, no events delivered")]
    RetryExhausted {
        /// Attempts made, including the first.
        attempts: u32,
        /// The failure that defeated the final attempt.
        last: Box<Self>,
    },
    /// The resume budget ran out with partial output already delivered.
    #[error(
        "resume budget exhausted after {resumes} resumes ({delivered_events} events delivered)"
    )]
    ResumeExhausted {
        /// Resumes attempted.
        resumes: u32,
        /// Events already delivered when the budget ran out.
        delivered_events: u64,
        /// The failure that defeated the final resume.
        last: Box<Self>,
    },
}

impl ProviderStreamError {
    /// Whether re-issuing the request can plausibly help (transport-class
    /// failures). Rejections, identity mismatches, and protocol violations
    /// surface immediately instead.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Http { status, .. } => *status >= 500 || *status == 429,
            Self::Disconnected { .. } | Self::IdleTimedOut { .. } => true,
            Self::Rejected(_)
            | Self::IdentityMismatch { .. }
            | Self::InvalidArguments { .. }
            | Self::UnnamedToolCall { .. }
            | Self::StreamError { .. }
            | Self::RetryExhausted { .. }
            | Self::ResumeExhausted { .. } => false,
        }
    }
}
