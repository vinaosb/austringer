//! Max-requests circuit breaker (plan todo 27): auto-approved loops
//! trip a typed breaker — N auto-approvals inside a sliding window
//! force subsequent requests to Ask. Yolo does not bypass the breaker
//! (stopping runaway auto-approval is its entire purpose).

use std::sync::Mutex;

/// Time source for the breaker (deterministic in tests).
pub trait Clock: Send + Sync {
    /// Monotonic-ish milliseconds.
    fn now_ms(&self) -> u64;
}

/// Wall-clock source.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Scriptable clock for tests.
#[derive(Debug, Default)]
pub struct FakeClock {
    now_ms: Mutex<u64>,
}

impl FakeClock {
    /// A clock starting at 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances and returns the new time.
    pub fn advance(&self, ms: u64) -> u64 {
        if let Ok(mut guard) = self.now_ms.lock() {
            *guard += ms;
        }
        self.now_ms()
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.now_ms.lock().map(|g| *g).unwrap_or(0)
    }
}

/// Breaker tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakerConfig {
    /// Auto-approvals allowed inside the window.
    pub max_requests: usize,
    /// Window length in milliseconds.
    pub window_ms: u64,
}

impl BreakerConfig {
    /// A live breaker.
    #[must_use]
    pub fn new(max_requests: usize, window_ms: u64) -> Self {
        Self {
            max_requests,
            window_ms,
        }
    }

    /// A breaker that never trips (explicit opt-out).
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            max_requests: usize::MAX,
            window_ms: u64::MAX,
        }
    }
}

/// Sliding-window breaker state.
#[derive(Debug)]
pub struct BreakerState {
    config: BreakerConfig,
    allowed_at: Vec<u64>,
}

impl BreakerState {
    /// Fresh, untripped state.
    #[must_use]
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            config,
            allowed_at: Vec::new(),
        }
    }

    /// Whether the breaker is tripped at `now_ms`.
    #[must_use]
    pub fn tripped(&self, now_ms: u64) -> bool {
        self.recent(now_ms) >= self.config.max_requests
    }

    /// Auto-approvals counted inside the window at `now_ms`.
    #[must_use]
    pub fn observed(&self, now_ms: u64) -> usize {
        self.recent(now_ms)
    }

    /// Records one auto-approval.
    pub fn record_allow(&mut self, now_ms: u64) {
        self.allowed_at.push(now_ms);
        self.allowed_at
            .retain(|t| *t + self.config.window_ms > now_ms);
    }

    fn recent(&self, now_ms: u64) -> usize {
        self.allowed_at
            .iter()
            .filter(|t| **t + self.config.window_ms > now_ms)
            .count()
    }
}
