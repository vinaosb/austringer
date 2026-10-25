//! The routing outcome vocabulary (plan todo 42): modeled stop reasons
//! (end-turn / refusal-truncation / max-tokens), wire failures, the
//! decision space, terminal failures, the surfaced audit events, and the
//! mismatched-acknowledgement rejection.

use std::time::Duration;

use austringer_providers::event::FinishReason;
use austringer_providers::identity::FallbackRequest;

/// Why a model turn ended, in routing terms (the modeled stop reasons:
/// end-turn, refusal-truncation, max-tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The model finished of its own accord (tool-call finishes are
    /// normal continuations).
    EndTurn,
    /// The model refused to continue.
    RefusalTruncation,
    /// The output token limit truncated the turn.
    MaxTokens,
}

impl StopReason {
    /// Maps the provider finish vocabulary into the routing vocabulary.
    #[must_use]
    pub fn from_finish(reason: FinishReason) -> Self {
        match reason {
            FinishReason::EndTurn | FinishReason::ToolCall => Self::EndTurn,
            FinishReason::Refusal => Self::RefusalTruncation,
            FinishReason::Length => Self::MaxTokens,
        }
    }
}

/// How a dispatched turn failed on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchFailure {
    /// Rate limited; the server may hint a retry-after.
    RateLimit {
        /// Server-provided retry-after hint.
        retry_after: Option<Duration>,
    },
    /// The endpoint is unreachable / hard-failing.
    Outage,
}

/// What the router observes about one dispatched turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcome {
    /// The turn completed; why it stopped decides retries.
    Completed {
        /// The modeled stop reason.
        stop: StopReason,
    },
    /// The turn failed on the wire.
    Failed {
        /// How it failed.
        failure: DispatchFailure,
    },
}

/// What the router decides after observing a turn.
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackDecision {
    /// Nothing to do.
    Proceed,
    /// Retry the SAME model (attempt counter included).
    Retry {
        /// The attempt number about to run.
        attempt: u32,
    },
    /// Wait out a rate limit, then retry the SAME model.
    WaitOut {
        /// The bounded window (<= the 75s cap).
        window: Duration,
        /// The attempt number about to run.
        attempt: u32,
    },
    /// Propose the next verbatim hop. Inert until acknowledged — no
    /// request may target the hop before the user consents.
    AskFallback(FallbackRequest),
    /// Typed terminal failure (chain exhausted / unconfigured /
    /// declined).
    Fail(RoutingFailure),
}

/// Terminal routing failures — the router ran out of honest options.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RoutingFailure {
    /// Every hop of the origin model's chain was acknowledged and failed.
    #[error("fallback chain for {origin:?} exhausted ({chain:?}) — every hop failed")]
    ChainExhausted {
        /// The launch model whose chain walked out.
        origin: String,
        /// The verbatim chain, in order.
        chain: Vec<String>,
    },
    /// The model has no fallback chain configured.
    #[error("no fallback chain configured for {model:?}")]
    NoFallbackConfigured {
        /// The chainless model.
        model: String,
    },
    /// The user declined the proposed hop.
    #[error("fallback from {from:?} to {to:?} declined — staying on the original route")]
    Declined {
        /// The model switched away from.
        from: String,
        /// The hop that was refused.
        to: String,
    },
}

/// One surfaced routing event (the every-hop-visible audit trail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingEvent {
    /// A hop was proposed and awaits confirmation.
    HopProposed {
        /// The task.
        task: String,
        /// Switched away from.
        from: String,
        /// Proposed hop.
        to: String,
    },
    /// A hop was confirmed.
    HopAcknowledged {
        /// The task.
        task: String,
        /// Switched away from.
        from: String,
        /// Confirmed hop.
        to: String,
    },
    /// A hop was declined.
    HopDeclined {
        /// The task.
        task: String,
        /// Switched away from.
        from: String,
        /// Declined hop.
        to: String,
    },
    /// A chain walked out.
    ChainExhausted {
        /// The task.
        task: String,
        /// The chain's origin model.
        origin: String,
    },
    /// A rate-limit wait-out window started.
    WaitOut {
        /// The task.
        task: String,
        /// The model waiting.
        model: String,
        /// The window in seconds.
        window_secs: u64,
    },
}

impl RoutingEvent {
    /// Stable event kind (`"hop_proposed"`, ...).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::HopProposed { .. } => "hop_proposed",
            Self::HopAcknowledged { .. } => "hop_acknowledged",
            Self::HopDeclined { .. } => "hop_declined",
            Self::ChainExhausted { .. } => "chain_exhausted",
            Self::WaitOut { .. } => "wait_out",
        }
    }

    /// The switched-away model (`""` when the event has none).
    #[must_use]
    pub fn from(&self) -> &str {
        match self {
            Self::HopProposed { from, .. }
            | Self::HopAcknowledged { from, .. }
            | Self::HopDeclined { from, .. } => from,
            Self::ChainExhausted { origin, .. } => origin,
            Self::WaitOut { model, .. } => model,
        }
    }

    /// The proposed/confirmed/declined hop (`""` when the event has none).
    #[must_use]
    pub fn to(&self) -> &str {
        match self {
            Self::HopProposed { to, .. }
            | Self::HopAcknowledged { to, .. }
            | Self::HopDeclined { to, .. } => to,
            Self::ChainExhausted { .. } | Self::WaitOut { .. } => "",
        }
    }
}

/// An acknowledgement that does not match the pending proposal.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("acknowledgement for {got:?} does not match the pending proposal {expected:?}")]
pub struct FallbackRejected {
    /// The hop the proposal named.
    pub expected: String,
    /// The hop the acknowledgement named.
    pub got: String,
}
