//! Retry policy and seeded jitter.
//!
//! Retry is IDEMPOTENCY-SAFE ONLY BEFORE the first event reaches the
//! consumer: a transparent jittered re-issue is legal exactly while nothing
//! has been emitted downstream (the consumer observed nothing, so a
//! re-issue is observationally a fresh attempt). After the first delivered
//! event, failures escalate to RESUME — re-issue plus offset skip plus the
//! tool-call dedup registry — never a silent replay. The boundary itself
//! lives in [`crate::turn`]; this module is just the timing math.

use std::time::Duration;

/// Jitter sample source. Injected so tests are deterministic (fixed
/// sequences) and production is seeded.
pub trait JitterSource: Send {
    /// Next jitter factor in the half-open window `[0, 1)`.
    fn next_jitter(&mut self) -> f64;
}

/// Deterministic xorshift64* PRNG: seeded jitter without pulling in `rand`.
pub struct SeededJitter {
    state: u64,
}

impl SeededJitter {
    /// Creates a jitter source from a seed. Identical seeds produce
    /// identical sequences; the zero seed is folded to a fixed nonzero
    /// state (xorshift64* has a zero fixpoint).
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15 | 1,
        }
    }
}

impl JitterSource for SeededJitter {
    fn next_jitter(&mut self) -> f64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let scrambled = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (scrambled >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Test-injected jitter: replays a fixed sequence, then repeats its final
/// value forever.
pub struct FixedJitter {
    values: std::collections::VecDeque<f64>,
}

impl FixedJitter {
    /// Creates a jitter source replaying `values` in order.
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = f64>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }
}

impl JitterSource for FixedJitter {
    fn next_jitter(&mut self) -> f64 {
        match self.values.len() {
            0 => 0.5,
            1 => self.values[0],
            _ => self.values.pop_front().unwrap_or(0.5),
        }
    }
}

/// Exponential backoff with equal jitter: `base = initial *
/// multiplier^(failures-1)` capped at `max_backoff`; the actual sleep is
/// uniform in `[base/2, base)` — half fixed, half jittered (the AWS
/// "exponential backoff and jitter" equal-jitter scheme, which avoids both
/// thundering herds and zero-wait retries).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetryPolicy {
    /// Total attempts allowed, including the first (`>= 1`).
    pub max_attempts: u32,
    /// Backoff before the second attempt.
    pub initial_backoff: Duration,
    /// Upper bound on any single backoff.
    pub max_backoff: Duration,
    /// Exponential growth factor (`>= 1` keeps backoff growing).
    pub multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
            multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// The jittered backoff after `failures` consecutive failures
    /// (`failures >= 1`), before the next attempt.
    pub fn backoff_for(&self, failures: u32, jitter: &mut dyn JitterSource) -> Duration {
        let exponent = failures.saturating_sub(1).min(u32::from(u16::MAX)) as i32;
        let growth = self.multiplier.max(1.0).powi(exponent);
        let capped =
            (self.initial_backoff.as_secs_f64() * growth).min(self.max_backoff.as_secs_f64());
        let half = capped / 2.0;
        let jittered = half + half * jitter.next_jitter();
        Duration::from_secs_f64(jittered)
    }
}
