//! The typed context-window budget and the deterministic token proxy
//! (plan todo 12).
//!
//! [`ContextBudget`] is the per-model-profile input: a window size in
//! tokens plus the eviction-trigger percent (75% by doctrine), the
//! post-eviction target percent, and the pre-compact flush lead. Token
//! counts everywhere in this module use [`estimate_tokens`] — a
//! deterministic byte-proportional proxy (`ceil(bytes / 4)`). The real
//! tokenizer arrives with model profiles (todo 10's cost tables); until
//! then every budget decision is a pure function of bytes, which is what
//! makes the eviction SIM deterministic.

use super::fold::{ErrorObservation, ToolReturnTable};
use super::tiers::ActivePlan;
use crate::event::RecordedEvent;

/// Degenerate (unusable) budget/session shapes, rejected typed rather
/// than producing nonsense decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DegenerateInput {
    /// A zero-token window: every ratio divides by zero.
    #[error("context window budget must be greater than zero tokens")]
    ZeroWindow,
    /// An empty session: there is nothing to evict or protect.
    #[error("session history is empty")]
    EmptySession,
}

/// The per-model context-window budget: trigger, target, and flush lead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBudget {
    /// Total context window in tokens (from the model profile).
    pub window_tokens: u64,
    /// Eviction trigger as a percent of the window (doctrine: 75).
    pub trigger_percent: u64,
    /// Post-eviction target as a percent of the window (doctrine: 60).
    pub target_percent: u64,
    /// Salient-fact flush fires this many tokens BEFORE the trigger.
    pub flush_lead_tokens: u64,
}

impl ContextBudget {
    /// Doctrine defaults for the given window.
    #[must_use]
    pub fn for_window(window_tokens: u64) -> Self {
        Self {
            window_tokens,
            trigger_percent: 75,
            target_percent: 60,
            flush_lead_tokens: 2_048,
        }
    }

    /// The absolute trigger threshold in tokens.
    #[must_use]
    pub fn trigger_tokens(&self) -> u64 {
        self.window_tokens.saturating_mul(self.trigger_percent) / 100
    }

    /// The absolute post-eviction target in tokens.
    #[must_use]
    pub fn target_tokens(&self) -> u64 {
        self.window_tokens.saturating_mul(self.target_percent) / 100
    }

    /// The pre-compact memory-flush threshold in tokens (trigger minus
    /// lead; saturates at zero when the lead exceeds the trigger).
    #[must_use]
    pub fn flush_threshold_tokens(&self) -> u64 {
        self.trigger_tokens().saturating_sub(self.flush_lead_tokens)
    }

    /// The config with a different flush lead (tests and profiles).
    #[must_use]
    pub const fn with_flush_lead(mut self, lead: u64) -> Self {
        self.flush_lead_tokens = lead;
        self
    }

    /// The degenerate shape, if any (zero window only).
    #[must_use]
    pub fn degenerate(&self) -> Option<DegenerateInput> {
        if self.window_tokens == 0 {
            Some(DegenerateInput::ZeroWindow)
        } else {
            None
        }
    }
}

/// Deterministic token proxy: `ceil(byte_len / 4)`.
///
/// Byte-proportional on purpose — UTF-8 payload mass is what the window
/// actually pays for — and stable across platforms and runs, which a
/// tokenizer lookup is not guaranteed to be.
#[must_use]
pub fn estimate_tokens(text: &str) -> u64 {
    text.len().div_ceil(4) as u64
}

/// Total token estimate for a history slice: the sorted-key serialized
/// size of each event (`unwrap_or_default` matches the infallible
/// serialization precedent in `context::history` — `Event` always
/// serializes).
#[must_use]
pub fn events_tokens(history: &[RecordedEvent]) -> u64 {
    history
        .iter()
        .map(|recorded| {
            estimate_tokens(&serde_json::to_string(&recorded.event).unwrap_or_default())
        })
        .sum()
}

/// Everything one eviction turn consumes.
#[derive(Debug, Clone, Default)]
pub struct EvictionInput {
    /// LLM-visible history (the `[L1,L2)` composition input).
    pub history: Vec<RecordedEvent>,
    /// The Tier0 active plan, carried verbatim.
    pub plan: Option<ActivePlan>,
    /// Error observations this session (the slot keeps the latest).
    pub errors: Vec<ErrorObservation>,
    /// Authoritative return codes for every executed call.
    pub tool_returns: ToolReturnTable,
    /// Fixed context mass outside this module's control (system,
    /// schemas, repomap), supplied by the caller.
    pub overhead_tokens: u64,
}

impl EvictionInput {
    /// An input from history alone.
    #[must_use]
    pub fn new(history: Vec<RecordedEvent>) -> Self {
        Self {
            history,
            ..Self::default()
        }
    }

    /// Attaches the Tier0 plan.
    #[must_use]
    pub fn with_plan(mut self, plan: ActivePlan) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Attaches the return-code table.
    #[must_use]
    pub fn with_tool_returns(mut self, table: ToolReturnTable) -> Self {
        self.tool_returns = table;
        self
    }

    /// Attaches fixed overhead mass.
    #[must_use]
    pub fn with_overhead(mut self, tokens: u64) -> Self {
        self.overhead_tokens = tokens;
        self
    }

    /// Latest error observation by sequence number.
    #[must_use]
    pub fn latest_error(&self) -> Option<&ErrorObservation> {
        self.errors.iter().max_by_key(|error| error.seq.0)
    }

    /// Whole-context usage estimate: overhead + plan + history + error
    /// slot.
    #[must_use]
    pub fn used_tokens(&self) -> u64 {
        self.used_tokens_with(&self.history)
    }

    /// Usage estimate with the history replaced by a working projection
    /// (the walk measures itself against its working copy).
    #[must_use]
    pub fn used_tokens_with(&self, history: &[RecordedEvent]) -> u64 {
        let plan = self
            .plan
            .as_ref()
            .map_or(0, |plan| estimate_tokens(&plan.render()));
        let slot = self
            .latest_error()
            .map_or(0, |error| estimate_tokens(&error.message));
        self.overhead_tokens + plan + events_tokens(history) + slot
    }
}
