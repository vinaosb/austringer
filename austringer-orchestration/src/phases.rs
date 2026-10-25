//! /boost-style 3-phase decoupling (Antigravity SS21: strategy -> isolated
//! execution -> verification as separate agent phases), mission validator
//! milestones (Droid SS16: scrutiny + user-testing workers at task
//! boundaries, config-skippable), and the CLEAN-CONTEXT reviewer (Devin
//! SS22: zero shared context — the reviewer reasons backward from the
//! diff alone; attention math beats shared context).

use crate::authorize::{CommandExecutionPolicy, SpawnAuthzClass};
use crate::fork::{ForkPlan, ToolSurface, fork_child};
use crate::launch::ChildJob;
use crate::role::Tier;

/// The three decoupled phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoostPhase {
    /// Plan the approach before touching anything.
    Strategy,
    /// The isolated execution waves.
    Execution,
    /// Independent verification.
    Verification,
}

impl BoostPhase {
    /// Wire name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Strategy => "strategy",
            Self::Execution => "execution",
            Self::Verification => "verification",
        }
    }
}

impl std::fmt::Display for BoostPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Validator milestone kinds (Droid Missions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidatorKind {
    /// Adversarial scrutiny of the boundary's outputs.
    Scrutiny,
    /// User-testing simulation of the boundary's outputs.
    UserTesting,
}

impl ValidatorKind {
    /// Wire name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Scrutiny => "scrutiny",
            Self::UserTesting => "user-testing",
        }
    }
}

impl std::fmt::Display for ValidatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Builds the validator jobs for one task boundary: two separate
/// read-only explorer-tier children (standing authorization by
/// construction) cloned from the wave's resolved knobs.
pub fn validator_jobs(wave_jobs: &[&ChildJob], launch_id: &str) -> Vec<(ValidatorKind, ChildJob)> {
    let Some(base) = wave_jobs.first() else {
        return Vec::new();
    };
    let subjects: Vec<&str> = wave_jobs.iter().map(|j| j.child_id.as_str()).collect();
    [ValidatorKind::Scrutiny, ValidatorKind::UserTesting]
        .into_iter()
        .map(|kind| {
            let mut job = (*base).clone();
            job.child_id = format!("{launch_id}-child-v-{}", kind.name());
            job.role = "validator".to_string();
            job.tier = Tier::Explorer;
            job.writes = crate::write_set::WriteSet::default();
            job.policy = CommandExecutionPolicy::Off;
            job.authz = SpawnAuthzClass::Standing;
            job.objective = format!(
                "{} over {}: verify the boundary outputs independently",
                kind,
                subjects.join(", ")
            );
            (kind, job)
        })
        .collect()
}

/// The reviewer doctrine: the clean-context reviewer gets NO shared
/// context, only the diff manifest.
pub const REVIEW_DOCTRINE: &str = "You are an independent code reviewer. You have zero shared \
context with the authors: reason backward from the diff alone. Every claim must cite a diff line. \
Flag inconsistencies, unproven assumptions, and missing coverage.";

/// A clean-context review: the diff manifest plus (structurally discarded)
/// any parent context handed to the builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanContextReview {
    diff: String,
}

impl CleanContextReview {
    /// Builds the review from the diff manifest. The second parameter is
    /// the parent objective/context the caller is offering — it is
    /// DISCARDED BY CONSTRUCTION (the whole point of the pattern); it
    /// exists so the discard is explicit at the call site.
    #[must_use]
    pub fn from_artifacts(diff: &str, _parent_context_discarded: &str) -> Self {
        Self {
            diff: diff.to_string(),
        }
    }

    /// Structurally false: the review never carries parent context.
    pub fn parent_context_present(&self) -> bool {
        false
    }

    /// The diff manifest the reviewer reasons from.
    pub fn diff(&self) -> &str {
        &self.diff
    }

    /// Builds the reviewer's job: doctrine system prompt, diff-only
    /// objective, empty tool surface.
    #[must_use]
    pub fn reviewer_job(&self, launch_id: &str, summary_budget: usize) -> ReviewerJob {
        ReviewerJob {
            child_id: format!("{launch_id}-child-verification"),
            objective: self.diff.clone(),
            fork: fork_child(REVIEW_DOCTRINE, &ToolSurface::default(), &[]),
            summary_budget,
        }
    }
}

/// The clean-context reviewer's job description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerJob {
    /// Reviewer child id.
    pub child_id: String,
    /// The diff-only objective.
    pub objective: String,
    /// The doctrine prompt (never the parent prompt).
    pub fork: ForkPlan,
    /// Summary budget.
    pub summary_budget: usize,
}
