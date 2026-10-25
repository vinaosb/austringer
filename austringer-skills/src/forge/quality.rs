//! Trajectory quality filtering (Atropos-style export format): sessions
//! carry a typed export (messages + reward + metrics); the
//! [`QualityFilter`] gates the distiller — low-quality sessions never
//! distill, so garbage trajectories cannot become skill drafts.

use std::collections::BTreeMap;

/// Who emitted a trajectory turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TurnRole {
    /// User input.
    User,
    /// Model output.
    Assistant,
    /// Tool call/result.
    Tool,
}

/// One trajectory turn. `text` is UNTRUSTED input to the forge.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrajectoryTurn {
    /// Emitting role.
    pub role: TurnRole,
    /// Turn content (untrusted; may be any unicode).
    pub text: String,
}

/// How the task ended.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TaskOutcome {
    /// Task completed successfully.
    Success,
    /// Task failed (failed sessions never distill).
    Failure {
        /// Why it failed.
        reason: String,
    },
}

/// A distilled session shape: what happened, plus the prompts the
/// resulting skill must route on (replay basis) and must NOT route on.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionTrajectory {
    /// Originating session id.
    pub session_id: String,
    /// Turns in order (untrusted).
    pub turns: Vec<TrajectoryTurn>,
    /// Prompts the distilled skill MUST route on (eval replay set).
    pub originating_prompts: Vec<String>,
    /// Prompts the distilled skill must NOT route on.
    pub negative_prompts: Vec<String>,
    /// Task outcome.
    pub outcome: TaskOutcome,
}

/// Atropos-style typed export: trajectory + reward + metrics.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrajectoryExport {
    /// The trajectory itself.
    pub trajectory: SessionTrajectory,
    /// Scalar reward (higher is better; filter compares `>=`).
    pub reward: f64,
    /// Named auxiliary metrics (uninterpreted by the filter).
    pub metrics: BTreeMap<String, f64>,
}

impl TrajectoryExport {
    /// Wraps a trajectory with its reward.
    pub fn new(trajectory: SessionTrajectory, reward: f64) -> Self {
        Self {
            trajectory,
            reward,
            metrics: BTreeMap::new(),
        }
    }
}

/// Filter verdict for one export.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterDecision {
    /// The session may distill.
    Admit,
    /// The session is rejected; `reason` names the failing gate.
    Reject {
        /// Human-readable gate failure.
        reason: String,
    },
}

/// Minimum-quality gate applied before any model call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityFilter {
    /// Minimum scalar reward to admit (`>=`).
    pub min_reward: f64,
}

impl QualityFilter {
    /// A filter admitting exports with reward `>= min_reward` that ended
    /// in `Success` and carry at least one turn.
    pub fn new(min_reward: f64) -> Self {
        Self { min_reward }
    }

    /// Applies the gate.
    pub fn admits(&self, export: &TrajectoryExport) -> FilterDecision {
        if export.reward < self.min_reward {
            return FilterDecision::Reject {
                reason: format!("reward {} below minimum {}", export.reward, self.min_reward),
            };
        }
        if !matches!(export.trajectory.outcome, TaskOutcome::Success) {
            return FilterDecision::Reject {
                reason: "task outcome was a failure".to_string(),
            };
        }
        if export.trajectory.turns.is_empty() {
            return FilterDecision::Reject {
                reason: "empty trajectory".to_string(),
            };
        }
        FilterDecision::Admit
    }
}
