//! Per-agent telemetry (tokens, cost, duration, failure class) and the
//! 1-2k summary band enforced on every child result.

/// Why a child failed, in the launcher's own vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildFailureClass {
    /// The model errored (refusal, stream failure).
    Model,
    /// A tool call failed.
    Tool,
    /// The budget guard stopped the child.
    Budget,
    /// The child was canceled.
    Canceled,
    /// The external-backend protocol broke.
    Protocol,
    /// The sandbox rejected an operation.
    Sandbox,
}

impl ChildFailureClass {
    /// Stable wire name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Tool => "tool",
            Self::Budget => "budget",
            Self::Canceled => "canceled",
            Self::Protocol => "protocol",
            Self::Sandbox => "sandbox",
        }
    }
}

/// One child's measured behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChildTelemetry {
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Cache-read tokens.
    pub cache_read_tokens: u64,
    /// Cache-write tokens.
    pub cache_write_tokens: u64,
    /// Cost in micro-USD.
    pub cost_micro_usd: u64,
    /// Wall duration in milliseconds.
    pub duration_ms: u64,
    /// Failure class, if the child failed.
    pub failure_class: Option<ChildFailureClass>,
}

impl ChildTelemetry {
    /// Total tokens charged against the budget.
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_write_tokens)
    }
}

/// Lower bound of the summary band (1k).
pub const SUMMARY_MIN: usize = 1024;
/// Upper bound of the summary band (2k).
pub const SUMMARY_MAX: usize = 2048;
/// Omission marker for clamped summaries.
pub const TRUNCATION_MARKER: &str = " [truncated]";

/// Clamps a summary to `budget` bytes on a char boundary, marking the cut
/// (never a bare truncation).
pub fn clamp_summary(text: &str, budget: usize) -> String {
    let budget = budget.clamp(SUMMARY_MIN, SUMMARY_MAX);
    if text.len() <= budget {
        return text.to_string();
    }
    let limit = budget.saturating_sub(TRUNCATION_MARKER.len());
    let mut cut = limit.min(text.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", &text[..cut], TRUNCATION_MARKER)
}

/// Aggregated launch telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TelemetryRollup {
    /// Children measured.
    pub children: usize,
    /// Summed input tokens.
    pub total_input: u64,
    /// Summed output tokens.
    pub total_output: u64,
    /// Summed total tokens.
    pub total_tokens: u64,
    /// Failed children.
    pub failures: usize,
}

/// Rolls per-child telemetry up into launch totals.
pub fn rollup(entries: &[ChildTelemetry]) -> TelemetryRollup {
    let mut out = TelemetryRollup {
        children: entries.len(),
        ..TelemetryRollup::default()
    };
    for entry in entries {
        out.total_input = out.total_input.saturating_add(entry.input_tokens);
        out.total_output = out.total_output.saturating_add(entry.output_tokens);
        out.total_tokens = out.total_tokens.saturating_add(entry.total_tokens());
        if entry.failure_class.is_some() {
            out.failures += 1;
        }
    }
    out
}
