//! Typed errors for the headless LSP client. Every failure mode reachable
//! from the public API maps to exactly one variant; untrusted server output
//! is parsed bounded and can only surface as [`LspError::MalformedFrame`] or
//! [`LspError::RequestFailed`], never as a panic.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

use crate::config::Language;

/// All fallible `harness-lsp` operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LspError {
    /// Underlying process or pipe I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// No server is configured for the language (or file extension).
    #[error("no language server configured for {detail}")]
    NoServer {
        /// What was asked for (language name or file extension).
        detail: String,
    },

    /// The configured server program could not be spawned (typically: not
    /// on the user's PATH — availability is the user environment's call).
    #[error("language server for {language:?} unavailable: cannot spawn `{program}`")]
    ServerUnavailable {
        /// Language whose server failed to spawn.
        language: Language,
        /// Program that was attempted.
        program: String,
    },

    /// The server never answered `initialize` within the configured budget.
    /// The process was killed and the slot's crash counter was bumped.
    #[error("server for {language:?} did not answer initialize within {waited:?}")]
    InitializeTimeout {
        /// Language whose server hung.
        language: Language,
        /// How long the harness waited before giving up.
        waited: Duration,
    },

    /// The server accepted a request but never answered within the
    /// per-request budget (hung server).
    #[error("request {method} timed out after {waited:?}")]
    RequestTimeout {
        /// LSP method that hung.
        method: &'static str,
        /// How long the harness waited.
        waited: Duration,
    },

    /// The server answered with a JSON-RPC error object.
    #[error("request failed ({code}): {message}")]
    RequestFailed {
        /// JSON-RPC error code.
        code: i64,
        /// Server-provided message.
        message: String,
    },

    /// The byte stream violated the LSP base protocol (garbage, lying
    /// Content-Length, runaway header). Reads are bounded — this error is
    /// how "never OOM" surfaces.
    #[error("malformed frame: {detail}")]
    MalformedFrame {
        /// What exactly was wrong.
        detail: String,
    },

    /// The IDE-bridge provider is registered but not wired yet (todo 38).
    #[error("IDE bridge endpoint registered but not implemented yet (plan todo 38)")]
    BridgeNotWired,

    /// A document exceeds the configured text cap (didOpen/didChange carry
    /// full text; oversized files are refused rather than buffered).
    #[error("file {path} is {size_bytes} bytes, over the {cap_bytes} byte text cap")]
    FileTooLarge {
        /// The offending file.
        path: PathBuf,
        /// Its size.
        size_bytes: u64,
        /// The configured cap.
        cap_bytes: u64,
    },
}

/// Why a language's navigation degraded to the lexical-search fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DegradeReason {
    /// The server crashed more times than the restart budget allows.
    #[error("restart budget exhausted after {crashes} crashes")]
    RestartBudgetExhausted {
        /// Total observed crashes for this language.
        crashes: u32,
    },

    /// The RSS guard killed the server over the memory cap and the restart
    /// budget is now exhausted.
    #[error("RSS {rss_bytes} bytes exceeded cap {cap_bytes} and restarts are exhausted")]
    RssCapExceeded {
        /// Sampled resident set size.
        rss_bytes: u64,
        /// Configured cap.
        cap_bytes: u64,
    },
}
