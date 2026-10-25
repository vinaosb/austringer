//! Budget ceilings and pacing (plan todo 25).
//!
//! Four mechanisms, one vocabulary:
//!
//! - **Ceilings** ([`caps`]): micro-USD and token caps per
//!   task/session/day period, config-shaped and validated (negative caps
//!   and inverted period stacks are typed rejections; a `Some(0)` cap
//!   means refuse-everything and fails inference at the ledger's
//!   [`SpendLedger::preflight`] gate).
//! - **Thresholds + signals** ([`ledger`], [`signal`]): warn at exactly
//!   80% of a cap, hard-stop at 100%, evaluated by cross-multiplied
//!   integer math (no rounding, never early or late). [`BudgetSignal`]
//!   carries the graceful-turn semantics — a Stop is delivered after the
//!   triggering spend is fully booked, so the consumer finishes the
//!   current tool and then ends the turn; never mid-tool. Hitting a token
//!   budget lands the typed [`IncompleteBudget`] state that resumes
//!   exactly at its sequence cursor — never a hard failure.
//! - **Credit rollover** ([`rollup`]): the orchestration tree aggregates
//!   child-agent spend; each subagent's subtree total is visible at the
//!   parent (u128, no floats).
//! - **Paced quotas** ([`quota`], [`governor`]): pools with burn-rate
//!   weights and conservation windows that pause expensive tiers while
//!   cheap ones stay healthy (deterministic, caller-injected ticks), plus
//!   the token-budget governor whose auto-degrade ladder
//!   (hold fan-out → collapse → serialize → solo) keys on remaining
//!   tokens BEFORE any per-call caps.
//!
//! The agent loop consumes [`BudgetSignal`] through
//! [`BudgetSignalSink`] at operation boundaries (wiring lands with the
//! loop todos; the signal type and semantics are fixed here). Core stays
//! provider-free: adapters map their per-response usage onto
//! [`Spend`](money::Spend) at their boundary (mapping documented there).

mod caps;
mod governor;
mod ledger;
mod money;
mod pacing;
mod quota;
mod resume;
mod rollup;
mod signal;

pub use caps::{BudgetCap, BudgetPeriod, BudgetSpec, InvalidSpec, RawCap};
pub use governor::{DegradeDecision, Governor, LadderThresholds, TaskClass};
pub use ledger::{LedgerError, SpendLedger};
pub use money::{Cost, MicroUsd, Spend, TokenCount};
pub use pacing::{Pacing, QuotaState};
pub use quota::{OpClass, PoolSpec, PoolTier, QuotaError, QuotaSpec, Weight};
pub use resume::{IncompleteBudget, ResumeError, ResumePlan};
pub use rollup::{RollupError, RollupNode};
pub use signal::{
    BudgetSignal, BudgetSignalSink, Dimension, Exhaustion, STOP_THRESHOLD_PCT, StopReason,
    ThresholdCrossed, WARN_THRESHOLD_PCT,
};
