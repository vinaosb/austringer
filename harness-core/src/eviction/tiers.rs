//! Tier classification and the protected zones (plan todo 12).
//!
//! The doctrine has three content tiers plus two positional immunities.
//! Tier0 (system core, active plan) never exists as evictable history —
//! targeting it is a typed rejection. The first ~3 messages (protected
//! head) and the last ~20 (protected tail) are eviction-immune regardless
//! of kind. The condensable middle splits by event kind: tool
//! calls/results are Tier2 (foldable to pointers), everything else is
//! Tier1 (condensable by the cheap-model seam).

use super::EvictionConfig;
use super::budget::DegenerateInput;
use crate::event::{Event, RecordedEvent};

/// The eviction tier vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Immutable content (system core, active plan). Never evicted;
    /// targeting it is a typed [`super::EvictionError::Tier0Protected`].
    Zero,
    /// Middle-zone conversation events (user/assistant/system):
    /// condensable via the cheap-model seam.
    One,
    /// Middle-zone tool calls/results: foldable to content pointers.
    Two,
    /// Positional immunity: the first `head_messages` events.
    ProtectedHead,
    /// Positional immunity: the last `tail_messages` events.
    ProtectedTail,
}

/// The Tier0 active plan block, carried verbatim through every
/// projection (never evicted, never condensed away).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePlan {
    /// The plan body (the T5 focus-chain / plan.md render).
    pub text: String,
}

impl ActivePlan {
    /// The stable tagged render of the plan.
    #[must_use]
    pub fn render(&self) -> String {
        format!("<active_plan>\n{}\n</active_plan>", self.text)
    }
}

/// Protected head/tail sizes (hermes: first ~3 + last ~20 messages are
/// eviction-immune; the middle is condensable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionConfig {
    /// Leading messages immune to eviction.
    pub head_messages: usize,
    /// Trailing messages immune to eviction.
    pub tail_messages: usize,
}

impl Default for ProtectionConfig {
    fn default() -> Self {
        Self {
            head_messages: 3,
            tail_messages: 20,
        }
    }
}

/// Zone classification over one session's LLM-visible history.
pub struct Projection<'a> {
    history: &'a [RecordedEvent],
    protection: ProtectionConfig,
}

impl<'a> Projection<'a> {
    /// Classifies `history` under `config`'s protection policy.
    #[must_use]
    pub fn new(history: &'a [RecordedEvent], config: &EvictionConfig) -> Self {
        Self {
            history,
            protection: config.protection,
        }
    }

    /// Number of classified events.
    #[must_use]
    pub fn total(&self) -> usize {
        self.history.len()
    }

    /// The tier/immunity zone of the event at `index` (`None` when out
    /// of bounds). Head immunity takes precedence over tail.
    #[must_use]
    pub fn zone(&self, index: usize) -> Option<Tier> {
        let recorded = self.history.get(index)?;
        if index < self.protection.head_messages {
            return Some(Tier::ProtectedHead);
        }
        if index + self.protection.tail_messages >= self.history.len() {
            return Some(Tier::ProtectedTail);
        }
        Some(match &recorded.event {
            Event::ToolCall { .. } | Event::ToolResult { .. } => Tier::Two,
            _ => Tier::One,
        })
    }

    /// Whether the event at `index` may be touched by any ladder stage.
    #[must_use]
    pub fn is_evictable(&self, index: usize) -> bool {
        matches!(self.zone(index), Some(Tier::One) | Some(Tier::Two))
    }

    /// The degenerate shape of this session/config, if any: an empty
    /// history or a zero window produce no meaningful projection.
    #[must_use]
    pub fn degenerate_of(
        history: &'a [RecordedEvent],
        config: &EvictionConfig,
    ) -> Option<DegenerateInput> {
        if history.is_empty() {
            Some(DegenerateInput::EmptySession)
        } else {
            config.budget.degenerate()
        }
    }

    /// Structural Tier0 guard: an eviction decision targeting `tier`
    /// content is a typed rejection, never a silent apply.
    pub fn guard_tier0(tier: &Tier) -> Result<(), super::EvictionError> {
        match tier {
            Tier::Zero => Err(super::EvictionError::Tier0Protected {
                target: "tier0".to_owned(),
            }),
            _ => Ok(()),
        }
    }
}
