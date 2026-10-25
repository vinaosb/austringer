//! The incomplete-status resume state (Antigravity §21): hitting a token
//! budget ends the task `status:incomplete` that resumes EXACTLY where it
//! stopped — never a hard failure.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::caps::{BudgetPeriod, BudgetSpec};
use super::ledger::{LedgerError, SpendLedger, threshold};
use super::signal::{Dimension, STOP_THRESHOLD_PCT, StopReason, WARN_THRESHOLD_PCT};
use crate::event::Sequence;

/// What a resume produces: the rebuilt ledger plus the sequence anchor the
/// loop continues from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePlan {
    /// The rebuilt ledger (consumption carried over).
    pub ledger: SpendLedger,
    /// The sequence anchor: resume exactly here.
    pub resume_seq: Sequence,
}

/// A typed resume failure (stale state).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResumeError {
    /// The persisted cursor references events beyond the log — the
    /// incomplete state is newer than the session log (torn restart).
    #[error("resume cursor {resume:?} is beyond the committed log ({last:?})")]
    CursorBeyondLog {
        /// The stale cursor.
        resume: Sequence,
        /// The last sequence the log actually reached.
        last: Sequence,
    },
    /// The persisted spec failed validation.
    #[error(transparent)]
    Invalid(#[from] LedgerError),
}

/// The persisted incomplete state (serde-stable for the session store).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncompleteBudget {
    /// Why the task went incomplete.
    pub reason: StopReason,
    /// The ceilings in force when the stop landed.
    pub caps: BudgetSpec,
    /// Money consumed per capped period.
    pub money: BTreeMap<BudgetPeriod, u128>,
    /// Tokens consumed per capped period.
    pub tokens: BTreeMap<BudgetPeriod, u64>,
    /// The sequence anchor: the last committed event when the stop landed.
    pub resume_seq: Sequence,
}

impl SpendLedger {
    /// Snapshots the resumable incomplete state at a stop: hitting a
    /// budget ends the task `status:incomplete` that resumes EXACTLY at
    /// `resume_seq` — never a hard failure.
    #[must_use]
    pub fn stop_snapshot(&self, reason: StopReason, resume_seq: Sequence) -> IncompleteBudget {
        IncompleteBudget {
            reason,
            caps: self.caps.clone(),
            money: self.money.clone(),
            tokens: self.tokens.clone(),
            resume_seq,
        }
    }

    /// Rebuilds a ledger after a restart. The cursor must lie within the
    /// committed log; consumption carries over (budgets never reset on
    /// resume) and already-fired thresholds are re-derived, not refired.
    pub fn resume(
        incomplete: &IncompleteBudget,
        last_committed: Sequence,
    ) -> Result<ResumePlan, ResumeError> {
        if incomplete.resume_seq > last_committed {
            return Err(ResumeError::CursorBeyondLog {
                resume: incomplete.resume_seq,
                last: last_committed,
            });
        }
        let mut ledger = SpendLedger::new(incomplete.caps.clone())?;
        let rederive =
            |period: BudgetPeriod, dim: Dimension, consumed: u128, ledger: &mut SpendLedger| {
                let Some(cap) = ledger.caps.cap(period).and_then(|c| c.value(dim)) else {
                    return;
                };
                if consumed >= threshold(cap, WARN_THRESHOLD_PCT) {
                    ledger.warned.insert((period, dim));
                }
                if consumed >= threshold(cap, STOP_THRESHOLD_PCT) {
                    ledger.stopped.insert((period, dim));
                }
            };
        for (period, consumed) in &incomplete.money {
            rederive(*period, Dimension::Money, *consumed, &mut ledger);
        }
        for (period, consumed) in &incomplete.tokens {
            rederive(
                *period,
                Dimension::Tokens,
                u128::from(*consumed),
                &mut ledger,
            );
        }
        ledger.money = incomplete.money.clone();
        ledger.tokens = incomplete.tokens.clone();
        Ok(ResumePlan {
            ledger,
            resume_seq: incomplete.resume_seq,
        })
    }
}
