//! Typed errors surfaced by the fake provider.

/// Every failure the fake provider can inject, as a typed error (no panics, no
/// string-matching in downstream assertions).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProviderError {
    /// HTTP-shaped failure (e.g. a scripted 500).
    #[error("provider returned HTTP {status} ({code}): {message}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Provider-specific error code.
        code: String,
        /// Human-readable message.
        message: String,
    },
    /// HTTP 200 but the completion payload contained zero choices.
    #[error("provider returned 200 OK but zero choices")]
    NoChoices,
    /// Output token limit reached before completion (`finish_reason=length`).
    #[error("output token limit reached before completion (finish_reason=length)")]
    OutputLimit,
    /// Request exceeded the model's context window (HTTP 413,
    /// `context_length_exceeded`).
    #[error("request exceeds the model context window (HTTP 413, context_length_exceeded)")]
    ContextLimit,
    /// No scripted rule matched the request — the script is missing a turn.
    #[error("no scripted rule matches request (model={model:?}, prompt={prompt:?})")]
    UnexpectedRequest {
        /// Offending request's model.
        model: String,
        /// Offending request's prompt.
        prompt: String,
    },
}
