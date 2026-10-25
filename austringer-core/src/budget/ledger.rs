//! The spend ledger: exact threshold evaluation and the preflight
//! inference gate. The incomplete-status resume state lives in
//! [`super::resume`].
//!
//! Threshold math is cross-multiplied in u128 — a signal fires iff
//! `consumed * 100 >= cap * pct` evaluated without division, so crossings
//! are exact for every integer cap (no rounding early/late). Each signal
//! fires exactly once per (period, dimension); consumption is monotonic,
//! so a crossing can never un-fire.

use std::collections::{BTreeMap, BTreeSet};

use super::caps::{BudgetPeriod, BudgetSpec, InvalidSpec};
use super::money::Spend;
use super::signal::{
    BudgetSignal, BudgetSignalSink, Dimension, Exhaustion, STOP_THRESHOLD_PCT, StopReason,
    ThresholdCrossed, WARN_THRESHOLD_PCT,
};

/// A typed ledger failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LedgerError {
    /// Booking the spend would overflow a consumption counter; the record
    /// is refused and the ledger state is unchanged.
    #[error("budget arithmetic overflow in {field} for {period} period")]
    Overflow {
        /// The period that would overflow.
        period: BudgetPeriod,
        /// The dimension that would overflow.
        field: &'static str,
    },
    /// The spec failed validation.
    #[error(transparent)]
    Invalid(#[from] InvalidSpec),
}

/// The integer threshold in consumption units: the smallest `consumed` for
/// which `consumed * 100 >= cap * pct` holds (i.e. ceil(cap * pct / 100)).
pub(crate) const fn threshold(cap: u128, pct: u32) -> u128 {
    let pct = pct as u128;
    let base = cap / 100;
    let rem = cap % 100;
    base * pct + (rem * pct).div_ceil(100)
}

/// Consumption tracking over a validated [`BudgetSpec`].
///
/// Every record counts toward every capped period simultaneously; each
/// period evaluates its own thresholds in canonical order (period asc,
/// money before tokens, warning before stop).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpendLedger {
    pub(crate) caps: BudgetSpec,
    pub(crate) money: BTreeMap<BudgetPeriod, u128>,
    pub(crate) tokens: BTreeMap<BudgetPeriod, u64>,
    pub(crate) warned: BTreeSet<(BudgetPeriod, Dimension)>,
    pub(crate) stopped: BTreeSet<(BudgetPeriod, Dimension)>,
}

impl SpendLedger {
    /// Creates a ledger over a validated spec.
    pub fn new(caps: BudgetSpec) -> Result<Self, LedgerError> {
        caps.validate()?;
        let (mut money, mut tokens) = (BTreeMap::new(), BTreeMap::new());
        for (period, cap) in caps.iter() {
            if cap.money.is_some() {
                money.insert(period, 0);
            }
            if cap.tokens.is_some() {
                tokens.insert(period, 0);
            }
        }
        Ok(Self {
            caps,
            money,
            tokens,
            warned: BTreeSet::new(),
            stopped: BTreeSet::new(),
        })
    }

    /// Books one spend and evaluates every capped period, delivering
    /// signals to `sink` in canonical order. On overflow nothing is booked
    /// and no signal fires (state unchanged).
    pub fn record(
        &mut self,
        spend: Spend,
        sink: &mut dyn BudgetSignalSink,
    ) -> Result<(), LedgerError> {
        let mut money_next = BTreeMap::new();
        for (period, current) in &self.money {
            let next = current
                .checked_add(spend.money().0)
                .ok_or(LedgerError::Overflow {
                    period: *period,
                    field: "money",
                })?;
            money_next.insert(*period, next);
        }
        let mut tokens_next = BTreeMap::new();
        for (period, current) in &self.tokens {
            let next = current
                .checked_add(spend.tokens.0)
                .ok_or(LedgerError::Overflow {
                    period: *period,
                    field: "tokens",
                })?;
            tokens_next.insert(*period, next);
        }
        for (period, next) in money_next {
            self.money.insert(period, next);
            self.evaluate(period, Dimension::Money, next, sink);
        }
        for (period, next) in tokens_next {
            let wide = u128::from(next);
            self.tokens.insert(period, next);
            self.evaluate(period, Dimension::Tokens, wide, sink);
        }
        Ok(())
    }

    /// Remaining budget in one dimension (`None` when uncapped; saturates
    /// at zero after an overshooting spend).
    #[must_use]
    pub fn remaining(&self, period: BudgetPeriod, dim: Dimension) -> Option<u128> {
        let cap = self.caps.cap(period)?.value(dim)?;
        let consumed = match dim {
            Dimension::Money => self.money.get(&period).copied(),
            Dimension::Tokens => self.tokens.get(&period).map(|t| u128::from(*t)),
        }?;
        Some(cap.saturating_sub(consumed))
    }

    /// The gate the loop calls before inference: refuses with the typed
    /// stop reason when any capped dimension is already at or over its
    /// cap. A `Some(0)` cap (dry-run) refuses immediately.
    pub fn preflight(&self) -> Result<(), StopReason> {
        for (period, cap) in self.caps.iter() {
            for dim in [Dimension::Money, Dimension::Tokens] {
                let Some(cap_value) = cap.value(dim) else {
                    continue;
                };
                let consumed = match dim {
                    Dimension::Money => self.money.get(&period).copied(),
                    Dimension::Tokens => self.tokens.get(&period).map(|t| u128::from(*t)),
                };
                if let Some(consumed) = consumed
                    && consumed >= cap_value
                {
                    return Err(self.exhaustion(period, dim, consumed, cap_value));
                }
            }
        }
        Ok(())
    }

    fn exhaustion(
        &self,
        period: BudgetPeriod,
        dim: Dimension,
        consumed: u128,
        cap: u128,
    ) -> StopReason {
        let exhaustion = Exhaustion {
            period,
            dimension: dim,
            consumed,
            cap,
        };
        match dim {
            Dimension::Money => StopReason::MoneyExhausted(exhaustion),
            Dimension::Tokens => StopReason::TokenExhausted(exhaustion),
        }
    }

    fn evaluate(
        &mut self,
        period: BudgetPeriod,
        dim: Dimension,
        consumed: u128,
        sink: &mut dyn BudgetSignalSink,
    ) {
        let Some(cap) = self.caps.cap(period).and_then(|c| c.value(dim)) else {
            return;
        };
        let warn_line = threshold(cap, WARN_THRESHOLD_PCT);
        let stop_line = threshold(cap, STOP_THRESHOLD_PCT);
        if !self.warned.contains(&(period, dim)) && consumed >= warn_line {
            self.warned.insert((period, dim));
            sink.on_signal(BudgetSignal::Warning(ThresholdCrossed {
                period,
                dimension: dim,
                threshold_pct: WARN_THRESHOLD_PCT,
                consumed,
                cap,
            }));
        }
        if !self.stopped.contains(&(period, dim)) && consumed >= stop_line {
            self.stopped.insert((period, dim));
            sink.on_signal(BudgetSignal::Stop(
                self.exhaustion(period, dim, consumed, cap),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LedgerError, SpendLedger, threshold};
    use crate::budget::caps::{BudgetCap, BudgetPeriod, BudgetSpec};
    use crate::budget::money::{MicroUsd, Spend, TokenCount};
    use crate::budget::signal::{BudgetSignal, BudgetSignalSink, Dimension};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Sink(Vec<BudgetSignal>);
    impl BudgetSignalSink for Sink {
        fn on_signal(&mut self, signal: BudgetSignal) {
            self.0.push(signal);
        }
    }

    #[test]
    fn threshold_is_exact_ceiling_for_uneven_caps() {
        // ceil(7 * 80 / 100) = 6: five tokens is under, six crosses.
        assert_eq!(threshold(7, 80), 6);
        assert_eq!(threshold(100, 80), 80);
        assert_eq!(threshold(100, 100), 100);
        assert_eq!(threshold(0, 80), 0);
    }

    #[test]
    fn uneven_cap_crosses_exactly() {
        let caps = BTreeMap::from([(
            BudgetPeriod::Task,
            BudgetCap {
                money: None,
                tokens: Some(TokenCount(7)),
            },
        )]);
        let made = SpendLedger::new(BudgetSpec::from_caps(caps));
        assert!(made.is_ok(), "valid spec rejected: {made:?}");
        let Ok(mut ledger) = made else {
            return;
        };
        let mut sink = Sink::default();
        assert!(ledger.record(Spend::unpriced(5), &mut sink).is_ok());
        assert!(sink.0.is_empty(), "5 of 7 is under 80%");
        assert!(ledger.record(Spend::unpriced(1), &mut sink).is_ok());
        assert_eq!(sink.0.len(), 1, "6 of 7 crosses exactly");
    }

    #[test]
    fn overflow_leaves_state_unchanged() {
        let caps = BTreeMap::from([(
            BudgetPeriod::Task,
            BudgetCap {
                money: Some(MicroUsd(u128::MAX - 5)),
                tokens: None,
            },
        )]);
        let made = SpendLedger::new(BudgetSpec::from_caps(caps));
        assert!(made.is_ok(), "valid spec rejected: {made:?}");
        let Ok(mut ledger) = made else {
            return;
        };
        let mut sink = Sink::default();
        assert!(
            ledger
                .record(Spend::priced(1, u128::MAX - 5), &mut sink)
                .is_ok()
        );
        let before = ledger.remaining(BudgetPeriod::Task, Dimension::Money);
        assert_eq!(
            ledger.record(Spend::priced(1, 10), &mut sink),
            Err(LedgerError::Overflow {
                period: BudgetPeriod::Task,
                field: "money"
            })
        );
        assert_eq!(
            ledger.remaining(BudgetPeriod::Task, Dimension::Money),
            before
        );
    }
}
