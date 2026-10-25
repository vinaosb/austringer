//! The per-parent-session delegation budget (todo 13): the fan-out cap
//! and the token aggregate across every task() call on ONE session.
//! BudgetGuard is per-launch with a 20M-token ceiling that is not a
//! cost guard — this is the explicit cross-call aggregate (T10/A6
//! storm discipline: hard caps plus explicit escalation criteria).

use austringer_orchestration::budget::{ABSOLUTE_MAX_CHILDREN, BudgetBreach};

/// Default fan-out cap per parent session (T10/A6: cap subagent storms;
/// escalation is the explicit `escalate_fan_out` call argument).
pub const DEFAULT_FAN_OUT_CAP: u32 = 4;
/// The escalated ceiling: explicit escalation raises the cap to the
/// orchestration crate's absolute children ceiling — still a hard cap.
pub const ESCALATED_FAN_OUT_CAP: u32 = ABSOLUTE_MAX_CHILDREN;

/// Fan-out beyond the cap without explicit escalation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "fan-out cap: {spawned} task children already spawned this session (cap {cap}); \
     explicit escalation: pass escalate_fan_out=true (hard ceiling {ESCALATED_FAN_OUT_CAP})"
)]
pub struct FanOutExceeded {
    /// Children already spawned this session.
    pub spawned: u32,
    /// The cap that was hit.
    pub cap: u32,
}

/// Read-only view of one parent session's delegation aggregate (todo
/// 15's surfacing input).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelegationSnapshot {
    /// Task children spawned this session.
    pub spawned: u32,
    /// Tokens spent across those children.
    pub tokens_spent: u64,
    /// The effective fan-out cap.
    pub fan_out_cap: u32,
    /// The per-parent token budget.
    pub token_budget: u64,
}

/// The PER-PARENT-SESSION delegation budget: the fan-out cap and the
/// token aggregate across every task() call on one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationBudget {
    fan_out_cap: u32,
    token_budget: u64,
    spawned: u32,
    tokens_spent: u64,
}

impl DelegationBudget {
    /// A budget with explicit caps.
    #[must_use]
    pub fn new(fan_out_cap: u32, token_budget: u64) -> Self {
        Self {
            fan_out_cap,
            token_budget,
            spawned: 0,
            tokens_spent: 0,
        }
    }

    /// Authorizes one spawn: the token aggregate refuses first (a spent
    /// session spawns nothing new), then the fan-out cap — which
    /// explicit escalation lifts to the absolute ceiling. Returns the
    /// spawn ordinal.
    ///
    /// # Errors
    /// [`super::TaskRefusal::Tokens`] or
    /// [`super::TaskRefusal::FanOut`].
    pub fn authorize(&mut self, escalate: bool) -> Result<u32, super::TaskRefusal> {
        if self.tokens_spent >= self.token_budget {
            return Err(BudgetBreach::TokensExhausted {
                spent: self.tokens_spent,
                budget: self.token_budget,
            }
            .into());
        }
        let cap = if escalate {
            self.fan_out_cap.max(ESCALATED_FAN_OUT_CAP)
        } else {
            self.fan_out_cap
        };
        if self.spawned >= cap {
            return Err(FanOutExceeded {
                spawned: self.spawned,
                cap,
            }
            .into());
        }
        self.spawned += 1;
        Ok(self.spawned)
    }

    /// Records token spend; the spend counts even when it breaches.
    ///
    /// # Errors
    /// [`BudgetBreach::TokensExhausted`] when the aggregate crossed the
    /// budget.
    pub fn record_spend(&mut self, tokens: u64) -> Result<(), BudgetBreach> {
        self.tokens_spent = self.tokens_spent.saturating_add(tokens);
        if self.tokens_spent > self.token_budget {
            return Err(BudgetBreach::TokensExhausted {
                spent: self.tokens_spent,
                budget: self.token_budget,
            });
        }
        Ok(())
    }

    /// Current state.
    #[must_use]
    pub fn snapshot(&self) -> DelegationSnapshot {
        DelegationSnapshot {
            spawned: self.spawned,
            tokens_spent: self.tokens_spent,
            fan_out_cap: self.fan_out_cap,
            token_budget: self.token_budget,
        }
    }
}
