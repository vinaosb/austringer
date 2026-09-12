//! The token-budget governor (research §33): the auto-degrade ladder
//! `hold fan-out -> collapse -> serialize -> solo`, keyed on the TOKEN
//! budget BEFORE any per-call caps (per-call budgets apply only after
//! this ladder has chosen a rung — the ordering is fixed by design).

use super::money::TokenCount;

/// Task classes the ladder keys on. Budgets vary ~10x across classes, so
/// the rung thresholds key on the class, never one global cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskClass {
    /// Long-horizon multi-agent research (highest token budget).
    DeepResearch,
    /// Default mixed work.
    Balanced,
    /// Short targeted edits (lowest token budget).
    QuickEdit,
}

/// One rung of the auto-degrade ladder, healthiest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradeDecision {
    /// Token budget healthy: hold the current fan-out shape (the level is
    /// carried for the HUD / wiring).
    HoldFanOut {
        /// The fan-out level being held.
        current: u32,
    },
    /// Collapse fan-out: cap live agents at 2 (agents above the cap drain
    /// naturally — nothing is aborted mid-flight).
    CollapseFanOut,
    /// Serialize: one agent at a time.
    Serialize,
    /// Solo: drop orchestration entirely — a single agent, cheapest path.
    Solo,
}

impl DegradeDecision {
    /// The parallelism ceiling the rung imposes (`None` = unrestricted).
    #[must_use]
    pub const fn parallelism_ceiling(&self) -> Option<u32> {
        match self {
            Self::HoldFanOut { .. } => None,
            Self::CollapseFanOut => Some(2),
            Self::Serialize | Self::Solo => Some(1),
        }
    }
}

/// The remaining-token rung thresholds for one task class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LadderThresholds {
    /// At or above: hold fan-out.
    pub hold: u64,
    /// At or above: collapse fan-out (below `hold`).
    pub collapse: u64,
    /// At or above: serialize (below `collapse`).
    pub serialize: u64,
}

/// The deterministic policy table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Governor;

impl Governor {
    /// The standard policy: deep research tolerates ~10x the remaining
    /// tokens of quick-edit at every rung.
    #[must_use]
    pub const fn standard() -> Self {
        Self
    }

    /// The thresholds for one class (the policy table).
    #[must_use]
    pub const fn thresholds(class: TaskClass) -> LadderThresholds {
        match class {
            TaskClass::DeepResearch => LadderThresholds {
                hold: 200_000,
                collapse: 50_000,
                serialize: 10_000,
            },
            TaskClass::Balanced => LadderThresholds {
                hold: 60_000,
                collapse: 15_000,
                serialize: 3_000,
            },
            TaskClass::QuickEdit => LadderThresholds {
                hold: 20_000,
                collapse: 5_000,
                serialize: 1_000,
            },
        }
    }

    /// The deterministic ladder decision from remaining tokens, current
    /// parallelism, and class. Boundaries are inclusive downward: a
    /// remaining budget exactly at a rung threshold takes that rung.
    #[must_use]
    pub const fn decide(
        &self,
        remaining: TokenCount,
        current_parallelism: u32,
        class: TaskClass,
    ) -> DegradeDecision {
        let t = Self::thresholds(class);
        if remaining.0 >= t.hold {
            DegradeDecision::HoldFanOut {
                current: current_parallelism,
            }
        } else if remaining.0 >= t.collapse {
            DegradeDecision::CollapseFanOut
        } else if remaining.0 >= t.serialize {
            DegradeDecision::Serialize
        } else {
            DegradeDecision::Solo
        }
    }
}

impl Default for Governor {
    fn default() -> Self {
        Self::standard()
    }
}

#[cfg(test)]
mod tests {
    use super::{DegradeDecision, Governor, TaskClass};

    #[test]
    fn class_spread_is_ten_x_at_every_rung() {
        let deep = Governor::thresholds(TaskClass::DeepResearch);
        let quick = Governor::thresholds(TaskClass::QuickEdit);
        assert_eq!(deep.hold, 10 * quick.hold);
        assert_eq!(deep.collapse, 10 * quick.collapse);
        assert_eq!(deep.serialize, 10 * quick.serialize);
    }

    #[test]
    fn ceilings_shrink_down_the_ladder() {
        assert_eq!(
            DegradeDecision::HoldFanOut { current: 3 }.parallelism_ceiling(),
            None
        );
        assert_eq!(
            DegradeDecision::CollapseFanOut.parallelism_ceiling(),
            Some(2)
        );
        assert_eq!(DegradeDecision::Serialize.parallelism_ceiling(), Some(1));
        assert_eq!(DegradeDecision::Solo.parallelism_ceiling(), Some(1));
    }
}
