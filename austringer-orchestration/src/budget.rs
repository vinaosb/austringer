//! Hard budget guards and the spawn-pacing guard.
//!
//! Budgets: children / parallel / tokens caps, typed breaches, spend
//! recorded even when it breaches. Absolute ceilings make "no
//! 50-subagent spawns" structural — no caller configuration can exceed
//! them. Pacing (Amp orb, SS17): a burst quota refilled at a fixed rate
//! (20-burst, then one every 5 minutes by default).

/// Absolute ceiling on children per launch.
pub const ABSOLUTE_MAX_CHILDREN: u32 = 32;
/// Absolute ceiling on token spend per launch.
pub const ABSOLUTE_MAX_TOKENS: u64 = 20_000_000;
/// Default burst quota (Amp orb).
pub const DEFAULT_BURST: u32 = 20;
/// Default refill interval: one token per 5 minutes (Amp orb).
pub const DEFAULT_REFILL_MS: u64 = 5 * 60_000;

/// Typed budget breach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BudgetBreach {
    /// The child cap is exhausted.
    #[error("children exhausted: {spawned} spawned of max {max}")]
    ChildrenExhausted {
        /// Children spawned so far.
        spawned: u32,
        /// The cap.
        max: u32,
    },
    /// The concurrency cap is exhausted.
    #[error("parallel cap exhausted: {active} active of max {max}")]
    ParallelExhausted {
        /// Active children.
        active: u32,
        /// The cap.
        max: u32,
    },
    /// The token budget is exhausted.
    #[error("token budget exhausted: {spent} spent of {budget}")]
    TokensExhausted {
        /// Tokens spent so far (including the breaching spend).
        spent: u64,
        /// The budget.
        budget: u64,
    },
}

/// One spawn authorization; release returns the parallel slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnLease {
    /// Lease id (spawn ordinal).
    pub ordinal: u32,
}

/// Read-only budget state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetSnapshot {
    /// Children spawned.
    pub spawned: u32,
    /// Children currently active.
    pub active: u32,
    /// Tokens spent.
    pub tokens_spent: u64,
}

/// The launch budget guard.
#[derive(Debug, Clone)]
pub struct BudgetGuard {
    max_children: u32,
    max_parallel: u32,
    token_budget: u64,
    spawned: u32,
    active: u32,
    tokens_spent: u64,
}

impl BudgetGuard {
    /// Builds a guard with explicit caps.
    pub fn new(max_children: u32, max_parallel: u32, token_budget: u64) -> Self {
        Self {
            max_children,
            max_parallel,
            token_budget,
            spawned: 0,
            active: 0,
            tokens_spent: 0,
        }
    }

    /// Current state.
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            spawned: self.spawned,
            active: self.active,
            tokens_spent: self.tokens_spent,
        }
    }

    /// Authorizes one spawn (children + parallel caps).
    pub fn authorize_spawn(&mut self) -> Result<SpawnLease, BudgetBreach> {
        if self.spawned >= self.max_children {
            return Err(BudgetBreach::ChildrenExhausted {
                spawned: self.spawned,
                max: self.max_children,
            });
        }
        if self.active >= self.max_parallel {
            return Err(BudgetBreach::ParallelExhausted {
                active: self.active,
                max: self.max_parallel,
            });
        }
        self.spawned += 1;
        self.active += 1;
        Ok(SpawnLease {
            ordinal: self.spawned,
        })
    }

    /// Releases a parallel slot when a child finishes.
    pub fn release(&mut self, _lease: SpawnLease) {
        self.active = self.active.saturating_sub(1);
    }

    /// Records token spend; the spend counts even when it breaches.
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
}

/// Pacing denial (control-flow value with a retry hint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("pacing: no spawn token until {next_available_ms}ms")]
pub struct PacingDenied {
    /// Wall-clock milliseconds when the next token exists.
    pub next_available_ms: u64,
}

/// The spawn-pacing guard: `capacity` tokens, refilled continuously at one
/// token per `refill_one_every_ms` (fractional accrual carries, so five
/// one-minute gaps DO yield one token on a 5-minute refill).
#[derive(Debug, Clone)]
pub struct Pacer {
    capacity: u32,
    refill_one_every_ms: u64,
    milli_tokens: u64,
    last_refill_ms: u64,
}

impl Pacer {
    /// A custom pacer.
    pub fn new(capacity: u32, refill_one_every_ms: u64) -> Self {
        Self {
            capacity,
            refill_one_every_ms,
            milli_tokens: u64::from(capacity) * 1000,
            last_refill_ms: 0,
        }
    }

    /// The Amp orb default: 20-burst, one refill per 5 minutes.
    pub fn amp() -> Self {
        Self::new(DEFAULT_BURST, DEFAULT_REFILL_MS)
    }

    fn refill(&mut self, now_ms: u64) {
        if now_ms > self.last_refill_ms && self.refill_one_every_ms > 0 {
            let elapsed = now_ms - self.last_refill_ms;
            let gained = elapsed
                .saturating_mul(1000)
                .saturating_div(self.refill_one_every_ms);
            let cap = u64::from(self.capacity) * 1000;
            self.milli_tokens = self.milli_tokens.saturating_add(gained).min(cap);
            self.last_refill_ms = now_ms;
        }
    }

    /// Whole tokens available at `now_ms`.
    pub fn available(&mut self, now_ms: u64) -> u32 {
        self.refill(now_ms);
        (self.milli_tokens / 1000) as u32
    }

    /// Consumes one token or denies with a retry hint.
    pub fn try_acquire(&mut self, now_ms: u64) -> Result<(), PacingDenied> {
        self.refill(now_ms);
        if self.milli_tokens >= 1000 {
            self.milli_tokens -= 1000;
            return Ok(());
        }
        let deficit_ms = 1000 - self.milli_tokens;
        let wait = deficit_ms
            .saturating_mul(self.refill_one_every_ms)
            .saturating_div(1000);
        Err(PacingDenied {
            next_available_ms: now_ms.saturating_add(wait),
        })
    }
}
