//! Money and token primitives for budget accounting — integer-only.
//!
//! The money unit is micro-USD (1e-6 USD), the same convention as the
//! providers' cost tables (T10); all budget arithmetic is integer math and
//! floats never appear in the money path.

use serde::{Deserialize, Serialize};

/// A cost figure in micro-USD (1e-6 USD).
///
/// Maps 1:1 onto the providers' `CostBreakdown::total_usd_micros` (u128).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct MicroUsd(pub u128);

impl MicroUsd {
    /// The zero cost.
    pub const ZERO: Self = Self(0);

    /// Checked addition — never overflows silently.
    #[must_use]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(sum) => Some(Self(sum)),
            None => None,
        }
    }
}

/// A count of tokens — every billed class summed (input, output, cache
/// read, cache write).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct TokenCount(pub u64);

impl TokenCount {
    /// The zero count.
    pub const ZERO: Self = Self(0);

    /// Checked addition — never overflows silently.
    #[must_use]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(sum) => Some(Self(sum)),
            None => None,
        }
    }
}

/// Whether a spend carried a dollar figure (the providers' priced /
/// unpriced vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cost {
    /// Cost under a known price entry.
    Priced(MicroUsd),
    /// Model outside every price table: token counts only. A cost is never
    /// invented for unpriced models — the money ceiling simply does not
    /// advance while the token ceiling still guards.
    Unpriced,
}

/// One spend event: tokens always, cost only when the model is priced.
///
/// # Mapping from the providers crate (T10)
///
/// `austringer_providers::event::Usage` maps here losslessly: `tokens` =
/// `input_tokens + output_tokens + cache_read_tokens + cache_write_tokens`;
/// `cost` = `Cost::Priced(MicroUsd(breakdown.total_usd_micros))` when the
/// cost table resolved the model, `Cost::Unpriced` otherwise (the
/// providers' `Priced::Unpriced` marker). Core never depends on the
/// providers crate — adapters construct [`Spend`] at their boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    /// Tokens consumed by the spend (all billed classes).
    pub tokens: TokenCount,
    /// The dollar figure, when the model is priced.
    pub cost: Cost,
}

impl Spend {
    /// A priced spend: `tokens` tokens for `micros` micro-USD.
    #[must_use]
    pub fn priced(tokens: u64, micros: u128) -> Self {
        Self {
            tokens: TokenCount(tokens),
            cost: Cost::Priced(MicroUsd(micros)),
        }
    }

    /// An unpriced spend: token counts only.
    #[must_use]
    pub fn unpriced(tokens: u64) -> Self {
        Self {
            tokens: TokenCount(tokens),
            cost: Cost::Unpriced,
        }
    }

    /// The money figure ([`MicroUsd::ZERO`] when unpriced).
    #[must_use]
    pub fn money(&self) -> MicroUsd {
        match self.cost {
            Cost::Priced(micros) => micros,
            Cost::Unpriced => MicroUsd::ZERO,
        }
    }

    /// Checked component-wise addition (any overflow yields `None`).
    #[must_use]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let tokens = self.tokens.checked_add(rhs.tokens)?;
        let cost = match (self.cost, rhs.cost) {
            (Cost::Unpriced, Cost::Unpriced) => Cost::Unpriced,
            (Cost::Priced(a), Cost::Priced(b)) => {
                let sum = a.checked_add(b)?;
                Cost::Priced(sum)
            }
            (Cost::Priced(a), Cost::Unpriced) | (Cost::Unpriced, Cost::Priced(a)) => {
                Cost::Priced(a)
            }
        };
        Some(Self { tokens, cost })
    }
}

#[cfg(test)]
mod tests {
    use super::{Cost, MicroUsd, Spend, TokenCount};

    #[test]
    fn priced_spend_carries_both_dimensions() {
        let spend = Spend::priced(10, 250);
        assert_eq!(spend.tokens, TokenCount(10));
        assert_eq!(spend.money(), MicroUsd(250));
    }

    #[test]
    fn unpriced_spend_never_invents_cost() {
        let spend = Spend::unpriced(10);
        assert_eq!(spend.cost, Cost::Unpriced);
        assert_eq!(spend.money(), MicroUsd::ZERO);
    }

    #[test]
    fn checked_add_sums_tokens_and_money() {
        let sum = Spend::priced(10, 100)
            .checked_add(Spend::unpriced(5))
            .unwrap_or(Spend::unpriced(0));
        assert_eq!(sum.tokens, TokenCount(15));
        assert_eq!(sum.money(), MicroUsd(100));
    }

    #[test]
    fn checked_add_reports_overflow() {
        let max = Spend::priced(1, u128::MAX);
        assert!(max.checked_add(Spend::priced(1, 1)).is_none());
    }
}
