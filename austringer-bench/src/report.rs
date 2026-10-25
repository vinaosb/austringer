//! Gate report artifacts: measured numbers per scenario + verdicts + git
//! provenance that distinguishes clean vs dirty worktrees.

use serde::{Deserialize, Serialize};

/// Dirty iff `git status --porcelain` produced any non-empty line. Pure —
/// the caller owns spawning git (tests pin the parsing).
pub fn parse_porcelain(output: &str) -> bool {
    output.lines().any(|l| !l.trim().is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitMeta {
    /// False when git was unavailable (report never pretends provenance).
    pub captured: bool,
    pub head: Option<String>,
    pub dirty: bool,
}

impl GitMeta {
    pub const fn not_captured() -> Self {
        Self {
            captured: false,
            head: None,
            dirty: false,
        }
    }

    /// Capture HEAD + porcelain from the working tree. Any git failure
    /// degrades to [`GitMeta::not_captured`]; never an error.
    pub fn capture() -> Self {
        use std::process::Command;
        let run = |args: &[&str]| -> Option<String> {
            Command::new("git")
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| {
                    let s = String::from_utf8(o.stdout).ok()?;
                    let t = s.trim().to_owned();
                    (!t.is_empty()).then_some(t)
                })
        };
        match (run(&["rev-parse", "HEAD"]), run(&["status", "--porcelain"])) {
            (Some(head), status) => Self {
                captured: true,
                head: Some(head),
                dirty: status.as_deref().is_some_and(parse_porcelain),
            },
            (None, _) => Self::not_captured(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricStatus {
    Pass,
    Breach,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricRecord {
    pub name: String,
    /// MEASURED value (never a constant; gates compare this).
    pub value: f64,
    pub unit: String,
    pub status: MetricStatus,
    pub budget: Option<f64>,
    pub baseline: Option<f64>,
    pub regression_pct: Option<f64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioRecord {
    pub scenario: String,
    pub status: MetricStatus,
    pub metrics: Vec<MetricRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateReport {
    pub passed: bool,
    pub git: GitMeta,
    pub budgets_source: String,
    pub scenarios: Vec<ScenarioRecord>,
}

impl GateReport {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// One-line human summary.
    pub fn summary(&self) -> String {
        let breaches = self
            .scenarios
            .iter()
            .filter(|s| s.status == MetricStatus::Breach)
            .count();
        let skipped = self
            .scenarios
            .iter()
            .filter(|s| s.status == MetricStatus::Skipped)
            .count();
        format!(
            "{} scenarios, {} breaches, {} skipped, verdict {}",
            self.scenarios.len(),
            breaches,
            skipped,
            if self.passed { "PASS" } else { "FAIL" }
        )
    }
}
