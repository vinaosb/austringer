//! Structured, bounded gate errors — the compact rejection signal every
//! stage funnels through (file:line + primary message only, per the T19
//! bounded-output discipline; never a dump).

use std::path::PathBuf;

use super::error::{PreFlightFailure, Stage};

/// Maximum structured errors surfaced in one composite failure.
pub const MAX_SURFACED_ERRORS: usize = 8;

/// Maximum characters of a structured message (single line, capped).
pub const MAX_ERR_LINE_CHARS: usize = 200;

/// One machine-consumable gate error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredError {
    pub stage: Stage,
    pub tool: &'static str,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

/// First line of `raw`, capped at [`MAX_ERR_LINE_CHARS`]. The fallback
/// shape when a tool failed without parseable structure.
pub fn bounded_message(raw: &str) -> String {
    let first = raw.lines().next().unwrap_or_default();
    if first.chars().count() <= MAX_ERR_LINE_CHARS {
        first.to_owned()
    } else {
        let cut: String = first.chars().take(MAX_ERR_LINE_CHARS - 3).collect();
        format!("{cut}...")
    }
}

/// Assemble the single composite failure: primary first, bounded tail,
/// total count. An empty list (exit-code failure with nothing parsed)
/// still surfaces compactly as one bounded-line error.
pub fn build_failure(
    stage: Stage,
    mut errors: Vec<StructuredError>,
    iteration: usize,
) -> PreFlightFailure {
    if errors.is_empty() {
        errors.push(StructuredError {
            stage,
            tool: "preflight",
            path: None,
            line: None,
            column: None,
            message: "stage failed without structured output".to_owned(),
        });
    }
    let total = errors.len();
    errors.truncate(MAX_SURFACED_ERRORS);
    let mut errors = errors;
    let primary = errors.remove(0);
    PreFlightFailure {
        stage,
        primary,
        rest: errors,
        total_errors: total,
        produced_at_iteration: iteration,
    }
}

/// Wrap a tool's parsed extraction with the compact fallback: when a
/// nonzero-exit tool yields nothing parseable, one bounded line stands in
/// (deterministic, never a dump).
pub fn with_fallback(
    stage: Stage,
    tool: &'static str,
    parsed: Vec<StructuredError>,
    raw: &str,
) -> Vec<StructuredError> {
    if parsed.is_empty() && !raw.trim().is_empty() {
        vec![StructuredError {
            stage,
            tool,
            path: None,
            line: None,
            column: None,
            message: bounded_message(raw),
        }]
    } else {
        parsed
    }
}
