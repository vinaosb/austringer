//! Inference-effort vocabulary (plan todo 11).
//!
//! On cache-coupled providers (Anthropic renders reasoning effort and the
//! thinking budget INTO the prompt), changing either invalidates every
//! cache breakpoint (research SS33: hold-effort-constant discipline). The
//! builder therefore renders [`EffortSettings`] into the frozen system
//! segment — it may only change when a context epoch ends — while the
//! per-message beta channel passes effort as request metadata without
//! touching prompt bytes (the typed escape hatch adapters fill).

use serde::{Deserialize, Serialize};
use std::fmt;

use super::epoch::EpochId;

/// Coarse reasoning-effort level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffortLevel {
    /// Minimal deliberation.
    Low,
    /// Default deliberation.
    Medium,
    /// Maximum deliberation.
    High,
}

impl fmt::Display for EffortLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        })
    }
}

/// The two sanctioned channels for an effort change (hermes SS33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffortChannel {
    /// Durable: pinned at an epoch boundary (completed compaction or
    /// session movement), when the baseline is re-frozen anyway.
    EpochBoundary,
    /// Per-message beta channel: request metadata the adapter fills;
    /// never rendered into prompt bytes.
    PerMessageBeta,
}

/// The remedies carried by an [`EffortViolation`].
pub const EFFORT_REMEDIES: &[EffortChannel] =
    &[EffortChannel::EpochBoundary, EffortChannel::PerMessageBeta];

/// A mid-epoch effort change, rejected: on cache-coupled providers
/// effort renders INTO the prompt, so changing it mid-epoch would
/// invalidate every cache breakpoint.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct EffortViolation {
    /// The epoch whose effort is pinned.
    pub epoch: EpochId,
    /// The pinned settings.
    pub current: EffortSettings,
    /// The settings this turn requested.
    pub requested: EffortSettings,
    /// The sanctioned ways to make this change.
    pub remedies: &'static [EffortChannel],
}

impl fmt::Display for EffortViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "effort change rejected in epoch {} ({} -> {}): effort renders \
             into the cached prefix and would invalidate every breakpoint; \
             remedies: wait for a completed compaction (epoch boundary) or \
             use the per-message beta channel",
            self.epoch.0, self.current.level, self.requested.level
        )
    }
}

/// Pinned inference-effort settings rendered into the system segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffortSettings {
    /// Reasoning-effort level.
    pub level: EffortLevel,
    /// Thinking budget in tokens, when the provider models one.
    pub thinking_budget: Option<u32>,
}

impl Default for EffortSettings {
    fn default() -> Self {
        Self {
            level: EffortLevel::Medium,
            thinking_budget: None,
        }
    }
}

impl EffortSettings {
    /// The canonical lines rendered into the system segment (sorted
    /// declaration order; the budget line appears only when set).
    #[must_use]
    pub fn render_lines(&self) -> String {
        match self.thinking_budget {
            Some(budget) => format!("effort: {}\nthinking-budget: {budget}", self.level),
            None => format!("effort: {}", self.level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lines_cover_level_and_optional_budget() {
        let base = EffortSettings::default();
        assert_eq!(base.render_lines(), "effort: medium");
        let tuned = EffortSettings {
            level: EffortLevel::High,
            thinking_budget: Some(2048),
        };
        assert_eq!(tuned.render_lines(), "effort: high\nthinking-budget: 2048");
    }
}
