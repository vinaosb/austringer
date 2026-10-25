//! The typed test report (todo 17, ecosystem alignment): what
//! `run_tests` returns instead of raw runner text. Per-test
//! pass/fail/ignored statuses, per-test durations where the runner
//! reports them, aggregate counts, and the run's exit status —
//! everything a consumer (the model, the todo-18 completion gate,
//! a frontend test explorer) needs without re-parsing prose.
//!
//! Receipts: Zed #5242 (886 upvotes — structured test results as the
//! most-requested editor integration); Qwen Test Failure Explorer
//! (shipped 2026-10); JetBrains native test runner.

use serde::{Deserialize, Serialize};

/// One test's outcome. Carried as the wire word (`passed` / `failed` /
/// `ignored`) — the vocabulary every consumer switches on.
pub type TestStatus = String;

/// One test's entry in a [`TestReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestEntry {
    /// The test's name as the runner printed it (nextest prefixes the
    /// binary: `rtfx_a fails`; libtest prints the bare path: `fails`).
    pub name: String,
    /// `passed`, `failed`, or `ignored`.
    pub status: TestStatus,
    /// Per-test duration in milliseconds. `None` is honest absence:
    /// nextest's human output carries per-test timings; stable libtest
    /// text does not (only the suite total).
    pub duration_ms: Option<u64>,
}

/// Aggregate counts parsed from the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSummary {
    /// Tests that passed.
    pub passed: u32,
    /// Tests that failed.
    pub failed: u32,
    /// Tests ignored / skipped.
    pub ignored: u32,
}

impl TestSummary {
    /// Counts a report's entries by status.
    #[must_use]
    pub fn of(entries: &[TestEntry]) -> Self {
        let mut summary = Self {
            passed: 0,
            failed: 0,
            ignored: 0,
        };
        for entry in entries {
            match entry.status.as_str() {
                "passed" => summary.passed += 1,
                "failed" => summary.failed += 1,
                _ => summary.ignored += 1,
            }
        }
        summary
    }
}

/// A structured test-run result. Serialized as JSON — this IS the
/// `run_tests` tool observation, so the todo-18 gate (and any
/// frontend) consumes it without parsing text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestReport {
    /// The runner that produced this report: `cargo-nextest`,
    /// `cargo-libtest`, or `override` (the user-global command).
    pub framework: String,
    /// The command that ran (display form, program + args).
    pub command: String,
    /// The child's exit code (`None` when the platform reports no
    /// code, e.g. a signal death on Unix). A failing test is DATA
    /// here, not an error — nonzero exits with parsed entries are
    /// reported, not raised.
    pub exit_code: Option<i32>,
    /// The run hit its timeout ceiling and the tree was killed.
    pub timed_out: bool,
    /// Wall-clock duration of the whole run in milliseconds.
    pub suite_duration_ms: u64,
    /// Per-test entries, in runner output order.
    pub tests: Vec<TestEntry>,
    /// Aggregate counts.
    pub summary: TestSummary,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_statuses() {
        let entries = vec![
            TestEntry {
                name: "a".into(),
                status: "passed".into(),
                duration_ms: Some(1),
            },
            TestEntry {
                name: "b".into(),
                status: "failed".into(),
                duration_ms: None,
            },
            TestEntry {
                name: "c".into(),
                status: "ignored".into(),
                duration_ms: None,
            },
        ];
        assert_eq!(
            TestSummary::of(&entries),
            TestSummary {
                passed: 1,
                failed: 1,
                ignored: 1
            }
        );
    }

    #[test]
    fn report_round_trips_through_json() {
        let report = TestReport {
            framework: "cargo-nextest".into(),
            command: "cargo nextest run --no-fail-fast".into(),
            exit_code: Some(100),
            timed_out: false,
            suite_duration_ms: 512,
            tests: vec![TestEntry {
                name: "rtfx_a fails".into(),
                status: "failed".into(),
                duration_ms: Some(28),
            }],
            summary: TestSummary {
                passed: 0,
                failed: 1,
                ignored: 0,
            },
        };
        let json = serde_json::to_string(&report).expect("serialize");
        let back: TestReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, report);
        assert!(json.contains("\"duration_ms\":28"));
    }
}
