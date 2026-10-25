//! Threshold signals and the consumer seam.
//!
//! The ledger emits [`BudgetSignal`]s at record boundaries; the agent loop
//! (wired in a later todo through its ExitOnError/StopHook seams) receives
//! them via [`BudgetSignalSink`]. Delivery happens after the spend that
//! crossed a threshold is fully booked — which is precisely what makes the
//! stop graceful: the current tool's cost is already accounted, the
//! consumer finishes the tool, then ends the turn.

use serde::{Deserialize, Serialize};

use super::caps::BudgetPeriod;

/// The warning threshold, in percent of the cap (plan: warn at 80%).
pub const WARN_THRESHOLD_PCT: u32 = 80;
/// The hard-stop threshold, in percent of the cap (plan: stop at 100%).
pub const STOP_THRESHOLD_PCT: u32 = 100;

/// Which ceiling dimension a signal refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Dimension {
    /// The micro-USD ceiling.
    Money,
    /// The token ceiling.
    Tokens,
}

impl Dimension {
    /// Lowercase name used in messages.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Money => "money",
            Self::Tokens => "tokens",
        }
    }
}

/// The 80%-crossing notice: fired exactly once, at the first record whose
/// cumulative consumption reaches the threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThresholdCrossed {
    /// The period whose threshold was crossed.
    pub period: BudgetPeriod,
    /// The dimension that crossed.
    pub dimension: Dimension,
    /// The threshold that fired (80).
    pub threshold_pct: u32,
    /// Cumulative consumption after the crossing record.
    pub consumed: u128,
    /// The cap being consumed against.
    pub cap: u128,
}

/// A ceiling is exhausted: consumption reached 100% of the cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exhaustion {
    /// The period that exhausted.
    pub period: BudgetPeriod,
    /// The dimension that exhausted.
    pub dimension: Dimension,
    /// Cumulative consumption (may exceed the cap on an overshooting spend).
    pub consumed: u128,
    /// The cap that was reached.
    pub cap: u128,
}

/// Why a run must stop.
///
/// Graceful-turn semantics: the consumer finishes the current tool (the
/// triggering spend is already fully recorded), then ends the turn — a
/// Stop never aborts mid-tool.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum StopReason {
    /// The micro-USD ceiling is exhausted.
    #[error("money budget exhausted for {period} period: consumed {consumed} of {cap} micro-USD", period = .0.period, consumed = .0.consumed, cap = .0.cap)]
    MoneyExhausted(Exhaustion),
    /// The token ceiling is exhausted.
    #[error("token budget exhausted for {period} period: consumed {consumed} of {cap} tokens", period = .0.period, consumed = .0.consumed, cap = .0.cap)]
    TokenExhausted(Exhaustion),
}

impl StopReason {
    /// The exhaustion payload.
    #[must_use]
    pub const fn exhaustion(&self) -> &Exhaustion {
        match self {
            Self::MoneyExhausted(e) | Self::TokenExhausted(e) => e,
        }
    }
}

/// The typed signal the agent loop consumes at operation boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetSignal {
    /// Consumption crossed the warning threshold (80%).
    Warning(ThresholdCrossed),
    /// Consumption reached the hard-stop threshold (100%): finish the
    /// current tool, then end the turn.
    Stop(StopReason),
}

impl BudgetSignal {
    /// True for [`BudgetSignal::Stop`].
    #[must_use]
    pub const fn is_stop(&self) -> bool {
        matches!(self, Self::Stop(_))
    }

    /// The period the signal belongs to.
    #[must_use]
    pub const fn period(&self) -> BudgetPeriod {
        match self {
            Self::Warning(w) => w.period,
            Self::Stop(s) => match s {
                StopReason::MoneyExhausted(e) | StopReason::TokenExhausted(e) => e.period,
            },
        }
    }
}

/// The consumer seam: receives signals as spends are recorded. The loop's
/// implementation (later wiring) maps `Stop` onto its graceful-stop path —
/// finish the running tool, record its spend, end the turn.
pub trait BudgetSignalSink {
    /// Receives one signal, in emission order.
    fn on_signal(&mut self, signal: BudgetSignal);
}

#[cfg(test)]
mod tests {
    use super::{BudgetPeriod, BudgetSignal, Dimension, Exhaustion, StopReason};

    #[test]
    fn signal_accessors_report_period_and_stopness() {
        let exhaustion = Exhaustion {
            period: BudgetPeriod::Day,
            dimension: Dimension::Tokens,
            consumed: 10,
            cap: 10,
        };
        let stop = BudgetSignal::Stop(StopReason::TokenExhausted(exhaustion));
        assert!(stop.is_stop());
        assert_eq!(stop.period(), BudgetPeriod::Day);
        let warn = BudgetSignal::Warning(super::ThresholdCrossed {
            period: BudgetPeriod::Day,
            dimension: Dimension::Tokens,
            threshold_pct: 80,
            consumed: 8,
            cap: 10,
        });
        assert!(!warn.is_stop());
        assert_eq!(warn.period(), BudgetPeriod::Day);
    }
}
