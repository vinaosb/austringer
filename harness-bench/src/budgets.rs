//! Perf budgets of record (todo 43). Parsing is strict: malformed config
//! is a typed error, never a silent default.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BudgetConfigError {
    #[error("malformed budgets config: {0}")]
    Malformed(#[source] serde_json::Error),
    #[error("budget out of range: {field}={value} ({reason})")]
    OutOfRange {
        field: &'static str,
        value: u64,
        reason: &'static str,
    },
}

/// Absolute budgets enforced by the gate. All are lower-is-better.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetConfig {
    /// Process spawn-to-exit wall budget (min-of-N), plan §Verification.
    pub cold_start_ms: u64,
    /// Max renders while draining 100k no-op UI events (idle CPU 0.0%).
    pub idle_max_renders: u64,
    /// Resident-set ceiling, MB. Todo 43 binding figure: 80.
    /// (§Verification's 40 MB daemon aspiration is tracked in the daemon
    /// suite; the gate enforces the todo-43 number.)
    pub rss_mb: u64,
    /// p99 single-frame render budget (min-of-runs).
    pub p99_frame_ms: u64,
    /// Stripped release binary ceiling, MB.
    pub binary_mb: u64,
    /// Regression tolerance: current > baseline * (1 + pct/100) fails.
    pub regression_tolerance_pct: u64,
    /// Zed-style frame budget; any frame above it is a breach.
    pub frame_budget_ms: u64,
    /// Frames above this are hangs (separate classification).
    pub hang_threshold_ms: u64,
    /// Per-format media budget overrides (plan M28+): format key
    /// (FormatId snake_case) -> override table. Absent entries mean the
    /// generic budgets apply (incl. the <=1s pure round-trip line);
    /// today only avif encode carries one (rav1e is tens-of-seconds
    /// slow by design and is excluded from that line).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub media_format_overrides: BTreeMap<String, MediaFormatOverride>,
}

/// One format's media budget override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaFormatOverride {
    /// Encode wall-clock timeout in milliseconds.
    pub encode_timeout_ms: u64,
}

impl BudgetConfig {
    /// Todo 43 budgets: cold start <50 ms, idle 0.0% (<=1 render),
    /// RSS <80 MB, p99 <16 ms, binary <40 MB, >10% regression,
    /// 10 ms frame budget, 100 ms hang threshold.
    pub const fn plan_defaults() -> Self {
        Self {
            cold_start_ms: 50,
            idle_max_renders: 1,
            rss_mb: 80,
            p99_frame_ms: 16,
            binary_mb: 40,
            regression_tolerance_pct: 10,
            frame_budget_ms: 10,
            hang_threshold_ms: 100,
            media_format_overrides: BTreeMap::new(),
        }
    }

    /// Strict parse: every field required, unknown fields rejected,
    /// values validated.
    pub fn from_json_str(json: &str) -> Result<Self, BudgetConfigError> {
        let cfg: Self = serde_json::from_str(json).map_err(BudgetConfigError::Malformed)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn to_json(&self) -> Result<String, BudgetConfigError> {
        serde_json::to_string(self).map_err(BudgetConfigError::Malformed)
    }

    fn validate(&self) -> Result<(), BudgetConfigError> {
        use BudgetConfigError::OutOfRange;
        let positive = [
            ("cold_start_ms", self.cold_start_ms),
            ("rss_mb", self.rss_mb),
            ("p99_frame_ms", self.p99_frame_ms),
            ("binary_mb", self.binary_mb),
            ("frame_budget_ms", self.frame_budget_ms),
            ("hang_threshold_ms", self.hang_threshold_ms),
        ];
        for &(field, value) in &positive {
            if value == 0 {
                return Err(OutOfRange {
                    field,
                    value,
                    reason: "must be >= 1",
                });
            }
        }
        if self.idle_max_renders > 10 {
            return Err(OutOfRange {
                field: "idle_max_renders",
                value: self.idle_max_renders,
                reason: "idle budget sanity ceiling is 10 renders",
            });
        }
        if !(1..=100).contains(&self.regression_tolerance_pct) {
            return Err(OutOfRange {
                field: "regression_tolerance_pct",
                value: self.regression_tolerance_pct,
                reason: "must be 1..=100",
            });
        }
        Ok(())
    }
}
