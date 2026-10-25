//! Launch report types: outcome, early-stop reasons, telemetry rollup,
//! and the spec-source handle for the stale-reload guard.

use std::path::PathBuf;

use crate::authorize::SpawnRefusal;
use crate::budget::BudgetBreach;
use crate::journal::JournalError;
use crate::launch::{ChildFailure, ChildReport};
use crate::synthesis::SynthesisReport;
use crate::telemetry::TelemetryRollup;

/// Why a launch stopped early.
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    /// A budget cap was hit.
    Budget(BudgetBreach),
    /// Pacing denied a spawn.
    Pacing(crate::budget::PacingDenied),
    /// The spec mutated mid-run.
    StaleSpec {
        /// Fingerprint at launch.
        expected: String,
        /// Observed fingerprint.
        actual: String,
    },
}

/// Launch outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchOutcome {
    /// Every wave ran without early stop.
    Complete,
    /// Stopped early; partials reported.
    Partial,
}

/// The launch report.
#[derive(Debug, Clone)]
pub struct LaunchReport {
    /// Launch id.
    pub launch_id: String,
    /// Outcome.
    pub outcome: LaunchOutcome,
    /// Completed children.
    pub children: Vec<ChildReport>,
    /// Failed children.
    pub failures: Vec<ChildFailure>,
    /// Early-stop reason.
    pub stopped_early: Option<StopReason>,
    /// Waves started.
    pub waves_executed: u32,
    /// Family switches (surfaced, never silent).
    pub family_switches: Vec<(String, String)>,
    /// Synthesis (merged result + per-agent telemetry).
    pub synthesis: Option<SynthesisReport>,
    /// Spawned child ids.
    pub child_ids: Vec<String>,
    /// /boost phase trace.
    pub phase_trace: Vec<String>,
    /// Telemetry rollup.
    pub rollup: TelemetryRollup,
}

/// Launcher-level failure.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    /// Loud spawn refusal (fresh authorization missing).
    #[error(transparent)]
    SpawnRefusal(#[from] SpawnRefusal),
    /// Journal failure.
    #[error(transparent)]
    Journal(#[from] JournalError),
    /// I/O failure reading the spec source.
    #[error("io: {0}")]
    Io(String),
}

/// Resume failure.
#[derive(Debug, thiserror::Error)]
pub enum ResumeError {
    /// The child failed again.
    #[error("child failed during resume")]
    Failure(ChildFailure),
    /// No such child in the plan.
    #[error("no child for task `{0}`")]
    Missing(String),
    /// Journal failure.
    #[error(transparent)]
    Journal(#[from] JournalError),
}

/// A spec source file (stale-reload guard).
#[derive(Debug, Clone)]
pub struct SpecSource {
    /// Path.
    pub path: PathBuf,
}

impl From<PathBuf> for SpecSource {
    fn from(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SpecSource {
    /// Reads the current bytes.
    pub fn read(&self) -> Result<Vec<u8>, std::io::Error> {
        std::fs::read(&self.path)
    }
}
