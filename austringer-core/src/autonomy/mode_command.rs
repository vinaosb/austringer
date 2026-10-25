//! The `/mode` slash command (plan todo 8): the user-facing dial over the
//! shared [`AutonomyMatrix`].
//!
//! `/mode` reports the current state; `/mode auto|approve|chat|
//! smartapprove` switches the level (clearing the plan pairing and
//! syncing the artifact-review policy onto the plan gate when one was
//! wired in); `/mode plan` enters the read-only pairing. Unknown
//! arguments produce a deterministic usage notice; other slash commands
//! are delegated (`Unknown`) and plain text is left for Steer.

use async_trait::async_trait;

use crate::agent_loop::PlanGate;
use crate::agent_loop::stages::{SlashCommands, SlashFlow, StageCtx, StageError};
use crate::autonomy::matrix::{AutonomyHandle, AutonomyMode, ReviewPolicy};

/// The `/mode` command. Shares the matrix with the autonomy gate; the
/// optional [`PlanGate`] receives the review-policy sync.
pub struct ModeCommand {
    matrix: AutonomyHandle,
    plan_gate: Option<PlanGate>,
}

impl ModeCommand {
    /// Builds the command over the shared matrix and an optional
    /// artifact-review gate.
    #[must_use]
    pub fn new(matrix: AutonomyHandle, plan_gate: Option<PlanGate>) -> Self {
        Self { matrix, plan_gate }
    }

    fn describe(&self) -> String {
        self.matrix.read(|matrix| {
            format!(
                "mode: {} (plan: {}) — review: {} — toggles: {}",
                matrix.mode().name(),
                if matrix.is_plan() { "yes" } else { "no" },
                match matrix.review() {
                    ReviewPolicy::RequestReview => "request-review",
                    ReviewPolicy::AlwaysProceed => "always-proceed",
                },
                summarize_toggles(matrix)
            )
        })
    }

    fn sync_review(&self) {
        if let Some(gate) = &self.plan_gate {
            let review = self.matrix.read(|matrix| matrix.review());
            match review {
                ReviewPolicy::RequestReview => gate.hard_block(),
                ReviewPolicy::AlwaysProceed => gate.clear(),
            }
        }
    }
}

fn summarize_toggles(matrix: &crate::autonomy::matrix::AutonomyMatrix) -> String {
    use crate::autonomy::matrix::Capability;
    let on: Vec<&str> = Capability::ALL
        .iter()
        .filter(|capability| matrix.allows(**capability))
        .map(|capability| match capability {
            Capability::Read => "read",
            Capability::Edit => "edit",
            Capability::Exec => "exec",
            Capability::Compact => "compact",
            Capability::Context => "context",
            Capability::Search => "search",
            Capability::Completion => "completion",
            Capability::SecretRead => "secret-read",
            Capability::PairFold => "pair-fold",
            Capability::GrantMemory => "grant-memory",
        })
        .collect();
    if on.is_empty() {
        "none".to_owned()
    } else {
        on.join(",")
    }
}

#[async_trait]
impl SlashCommands for ModeCommand {
    async fn dispatch(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        command: &str,
    ) -> Result<SlashFlow, StageError> {
        if !command.starts_with('/') {
            return Ok(SlashFlow::NotACommand);
        }
        let Some(argument) = command.strip_prefix("/mode") else {
            return Ok(SlashFlow::Unknown);
        };
        let argument = argument.trim().to_lowercase();
        let notice = if argument.is_empty() {
            self.describe()
        } else if argument == "plan" {
            self.matrix.write(|matrix| matrix.enter_plan());
            "plan mode active: write tools are frozen at the dispatch gate".to_owned()
        } else if let Some(mode) = AutonomyMode::parse(&argument) {
            self.matrix.write(|matrix| matrix.set_mode(mode));
            format!("autonomy mode set: {}", mode.name())
        } else {
            format!("unknown mode '{argument}' (expected: auto, approve, chat, smartapprove, plan)")
        };
        // Re-sync on every handled /mode, including describe-only: the gate
        // must always reflect the matrix the user just saw or set.
        self.sync_review();
        Ok(SlashFlow::Handled {
            notice: Some(notice),
        })
    }
}
