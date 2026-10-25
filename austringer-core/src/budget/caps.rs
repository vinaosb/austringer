//! Ceiling specs: period → cap in both money and token dimensions.
//!
//! The config boundary parses [`RawCap`] (i128, negatives representable)
//! into validated [`BudgetCap`] values — negative caps are rejected typed,
//! never coerced. A `Some(0)` cap is legal and means "refuse everything"
//! (dry-run): the ledger's preflight gate refuses inference immediately.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::money::{MicroUsd, TokenCount};
use super::signal::Dimension;

/// The accounting period a ceiling applies to.
///
/// Ordered `Task < Session < Day`: an inner period's cap must never exceed
/// an enclosing period's cap in the same dimension (validated — the
/// "inverted periods" rejection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BudgetPeriod {
    /// One task run (resumes reset nothing — consumption carries over).
    Task,
    /// One session (spans many tasks).
    Session,
    /// One calendar day (spans many sessions).
    Day,
}

impl BudgetPeriod {
    /// Lowercase name used in messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Session => "session",
            Self::Day => "day",
        }
    }
}

impl std::fmt::Display for BudgetPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One period's ceilings. `None` = uncapped in that dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetCap {
    /// Micro-USD ceiling for the period.
    pub money: Option<MicroUsd>,
    /// Token ceiling for the period (all billed classes summed).
    pub tokens: Option<TokenCount>,
}

/// Validated ceilings: period → cap (the config-loaded shape).
///
/// Constructed via [`BudgetSpec::from_caps`] (no validation) or
/// [`BudgetSpec::try_from_raw`] (full validation); consumers that enforce
/// budgets (the ledger) validate on construction regardless.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BudgetSpec {
    /// Capped periods, iterated in period order.
    pub(crate) caps: BTreeMap<BudgetPeriod, BudgetCap>,
}

/// The unvalidated config-boundary form: i128 so negatives are
/// representable and rejectable typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawCap {
    /// Raw micro-USD ceiling (negative rejected typed).
    pub money: Option<i128>,
    /// Raw token ceiling (negative rejected typed).
    pub tokens: Option<i128>,
}

/// A typed spec rejection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidSpec {
    /// A negative ceiling in the named dimension.
    #[error("negative {field} cap for {period} period")]
    Negative {
        /// The offending period.
        period: BudgetPeriod,
        /// The offending dimension ("money" / "tokens").
        field: &'static str,
    },
    /// An inner period's cap exceeds its enclosing period's cap.
    #[error("{field} cap for {inner} period exceeds the enclosing {outer} period")]
    InvertedPeriods {
        /// The inner (nested) period.
        inner: BudgetPeriod,
        /// The enclosing period.
        outer: BudgetPeriod,
        /// The offending dimension.
        field: &'static str,
    },
}

impl BudgetCap {
    /// The cap value in one dimension, widened to u128.
    #[must_use]
    pub fn value(&self, dim: Dimension) -> Option<u128> {
        match dim {
            Dimension::Money => self.money.map(|m| m.0),
            Dimension::Tokens => self.tokens.map(|t| u128::from(t.0)),
        }
    }
}

impl BudgetSpec {
    /// Builds a spec from caps without validation (validate() or the
    /// ledger's constructor performs the checks).
    #[must_use]
    pub fn from_caps(caps: BTreeMap<BudgetPeriod, BudgetCap>) -> Self {
        Self { caps }
    }

    /// Parses and validates the config-boundary form. Negative values are
    /// rejected typed; the result is then checked for inverted periods.
    pub fn try_from_raw(raw: &BTreeMap<BudgetPeriod, RawCap>) -> Result<Self, InvalidSpec> {
        let mut caps = BTreeMap::new();
        for (period, raw_cap) in raw {
            let money = match raw_cap.money {
                Some(n) if n < 0 => {
                    return Err(InvalidSpec::Negative {
                        period: *period,
                        field: "money",
                    });
                }
                Some(n) => Some(MicroUsd(n as u128)),
                None => None,
            };
            let tokens = match raw_cap.tokens {
                Some(n) if n < 0 => {
                    return Err(InvalidSpec::Negative {
                        period: *period,
                        field: "tokens",
                    });
                }
                Some(n) => Some(TokenCount(n as u64)),
                None => None,
            };
            caps.insert(*period, BudgetCap { money, tokens });
        }
        let spec = Self { caps };
        spec.validate()?;
        Ok(spec)
    }

    /// Checks the inversion law: per dimension, `Task <= Session <= Day`
    /// wherever both sides of a pair are present.
    pub fn validate(&self) -> Result<(), InvalidSpec> {
        for dim in [Dimension::Money, Dimension::Tokens] {
            let value = |period: BudgetPeriod| -> Option<u128> {
                self.caps.get(&period).and_then(|cap| cap.value(dim))
            };
            for (inner, outer) in [
                (BudgetPeriod::Task, BudgetPeriod::Session),
                (BudgetPeriod::Session, BudgetPeriod::Day),
            ] {
                if let (Some(inner_val), Some(outer_val)) = (value(inner), value(outer))
                    && inner_val > outer_val
                {
                    return Err(InvalidSpec::InvertedPeriods {
                        inner,
                        outer,
                        field: dim.field(),
                    });
                }
            }
        }
        Ok(())
    }

    /// The cap for `period`, if any.
    #[must_use]
    pub fn cap(&self, period: BudgetPeriod) -> Option<&BudgetCap> {
        self.caps.get(&period)
    }

    /// All capped periods with their caps, in period order.
    pub fn iter(&self) -> impl Iterator<Item = (BudgetPeriod, &BudgetCap)> {
        self.caps.iter().map(|(period, cap)| (*period, cap))
    }
}

#[cfg(test)]
mod tests {
    use super::{BudgetCap, BudgetPeriod, BudgetSpec, InvalidSpec, RawCap};
    use crate::budget::money::{MicroUsd, TokenCount};
    use std::collections::BTreeMap;

    #[test]
    fn period_order_is_task_session_day() {
        assert!(BudgetPeriod::Task < BudgetPeriod::Session);
        assert!(BudgetPeriod::Session < BudgetPeriod::Day);
    }

    #[test]
    fn empty_spec_is_uncapped() {
        let spec = BudgetSpec::from_caps(BTreeMap::new());
        assert_eq!(spec.validate(), Ok(()));
        assert_eq!(spec.cap(BudgetPeriod::Day), None);
    }

    #[test]
    fn raw_zero_caps_parse_to_refuse_everything_caps() {
        let raw = BTreeMap::from([(
            BudgetPeriod::Task,
            RawCap {
                money: Some(0),
                tokens: Some(0),
            },
        )]);
        let spec = BudgetSpec::try_from_raw(&raw).unwrap_or_default();
        assert_eq!(
            spec.cap(BudgetPeriod::Task),
            Some(&BudgetCap {
                money: Some(MicroUsd(0)),
                tokens: Some(TokenCount(0))
            })
        );
    }

    #[test]
    fn inverted_task_over_day_is_caught_via_session() {
        let caps = BTreeMap::from([
            (
                BudgetPeriod::Task,
                BudgetCap {
                    money: None,
                    tokens: Some(TokenCount(10)),
                },
            ),
            (
                BudgetPeriod::Session,
                BudgetCap {
                    money: None,
                    tokens: Some(TokenCount(10)),
                },
            ),
            (
                BudgetPeriod::Day,
                BudgetCap {
                    money: None,
                    tokens: Some(TokenCount(5)),
                },
            ),
        ]);
        assert!(matches!(
            BudgetSpec::from_caps(caps).validate(),
            Err(InvalidSpec::InvertedPeriods { .. })
        ));
    }
}
