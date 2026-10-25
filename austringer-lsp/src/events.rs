//! Observable router lifecycle events (the QA/evidence channel): what the
//! router did to its servers and why. Consumed via
//! [`crate::router::LspRouter::take_events`].

use crate::config::Language;

/// One observable router lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterEvent {
    /// A language's server was spawned for the first time.
    ServerSpawned {
        /// Which language.
        language: Language,
    },
    /// A restart was scheduled; `wait` is the deterministic backoff.
    RestartScheduled {
        /// Which language.
        language: Language,
        /// Consecutive-crash count this retry is for.
        attempt: u32,
        /// The exact backoff about to be slept.
        wait: std::time::Duration,
    },
    /// A restart completed and the server is Ready again.
    ServerRestarted {
        /// Which language.
        language: Language,
    },
    /// A live server died (crash, hang, or guard kill).
    ServerCrashed {
        /// Which language.
        language: Language,
    },
    /// A language degraded to the lexical-search fallback.
    Degraded {
        /// Which language.
        language: Language,
        /// Why.
        reason: crate::error::DegradeReason,
    },
    /// The RSS guard killed a server over the memory cap.
    RssCapped {
        /// Which language.
        language: Language,
        /// Sampled resident set size.
        rss_bytes: u64,
        /// The configured cap.
        cap_bytes: u64,
    },
}
