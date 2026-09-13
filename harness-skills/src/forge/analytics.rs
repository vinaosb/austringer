//! Usage analytics + context-cost accounting: invocations, tokens spent,
//! hit-rate per skill, and typed pruning suggestions for underperformers.
//! Suggestions only — there is no auto-delete path anywhere in the forge.

use std::collections::BTreeMap;

/// Minimum invocations before hit-rate is considered meaningful.
pub const MIN_INVOCATIONS: u64 = 5;
/// Hit-rate below which a skill is a prune candidate.
pub const LOW_HIT_RATE: f64 = 0.2;
/// Token spend with zero hits that marks cost-without-value.
pub const HIGH_TOKENS_NO_VALUE: u64 = 100_000;

/// Accumulated usage for one skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UsageStats {
    /// Total invocations.
    pub invocations: u64,
    /// Total tokens spent loading/running the skill.
    pub tokens_spent: u64,
    /// Invocations that produced a useful (hit) outcome.
    pub hits: u64,
}

impl UsageStats {
    /// Hits ÷ invocations (0.0 when never invoked).
    pub fn hit_rate(&self) -> f64 {
        if self.invocations == 0 {
            0.0
        } else {
            self.hits as f64 / self.invocations as f64
        }
    }
}

/// Why a skill is a prune candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PruneReason {
    /// Invoked often enough but rarely useful.
    LowHitRate {
        /// The measured rate.
        rate: f64,
    },
    /// Expensive context cost with zero useful outcomes.
    CostWithoutValue {
        /// Tokens spent.
        tokens_spent: u64,
    },
}

/// One typed pruning suggestion.
#[derive(Debug, Clone, PartialEq)]
pub struct PruningSuggestion {
    /// The underperforming skill.
    pub skill: String,
    /// Why it is suggested.
    pub reason: PruneReason,
}

/// Per-skill usage analytics.
#[derive(Debug, Clone, Default)]
pub struct UsageAnalytics {
    per_skill: BTreeMap<String, UsageStats>,
}

impl UsageAnalytics {
    /// Empty analytics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one invocation (`hit` = the skill's content was actually
    /// used to produce the outcome).
    pub fn record_invocation(&mut self, skill: &str, tokens_spent: u64, hit: bool) {
        let stats = self.per_skill.entry(skill.to_string()).or_default();
        stats.invocations += 1;
        stats.tokens_spent += tokens_spent;
        stats.hits += u64::from(hit);
    }

    /// Accumulated stats for one skill.
    pub fn stats(&self, skill: &str) -> Option<&UsageStats> {
        self.per_skill.get(skill)
    }

    /// All tracked skills.
    pub fn skills(&self) -> impl Iterator<Item = (&String, &UsageStats)> {
        self.per_skill.iter()
    }

    /// Typed suggestions for underperformers (no side effects: the
    /// underlying stats and the skill itself are untouched).
    pub fn suggest_pruning(&self) -> Vec<PruningSuggestion> {
        self.per_skill
            .iter()
            .filter_map(|(skill, stats)| {
                if stats.invocations >= MIN_INVOCATIONS && stats.hit_rate() < LOW_HIT_RATE {
                    return Some(PruningSuggestion {
                        skill: skill.clone(),
                        reason: PruneReason::LowHitRate {
                            rate: stats.hit_rate(),
                        },
                    });
                }
                if stats.tokens_spent >= HIGH_TOKENS_NO_VALUE && stats.hits == 0 {
                    return Some(PruningSuggestion {
                        skill: skill.clone(),
                        reason: PruneReason::CostWithoutValue {
                            tokens_spent: stats.tokens_spent,
                        },
                    });
                }
                None
            })
            .collect()
    }
}
