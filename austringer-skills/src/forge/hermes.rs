//! Hermes five-mechanism loop (research §27), as composable stages —
//! never a single magic step:
//!
//! 1. [`HermesMechanism::InUseImprovement`] — mid-session correction
//!    capture (the correction journal) feeding later distillation;
//! 2. [`HermesMechanism::PostTurnReview`] — a post-turn background
//!    review fork that proposes drafts, sharing the caller's prefix
//!    cache discipline;
//! 3. [`HermesMechanism::IdleCurator`] — the lifecycle curator pass
//!    (stale/archive/pin suggestions) while idle;
//! 4. [`HermesMechanism::MemoryNudge`] — knowledge-suggestion updates
//!    from chat feedback;
//! 5. [`HermesMechanism::WriteApprovalGate`] — the human gate EVERY
//!    proposal from 1–4 must pass (draft tier + approval + eval), the
//!    seam implemented in [`crate::forge::gates`].
//!
//! Mechanisms 1–4 have zero write authority: their only output is
//! draft-tier proposals. Mechanism 5 is the only path to a routable
//! skill, and it requires an explicit human decision plus a passing
//! replay eval.

/// The five loop stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HermesMechanism {
    /// Mid-session correction capture feeding distillation.
    InUseImprovement,
    /// Post-turn background review fork (draft proposals).
    PostTurnReview,
    /// Idle curator lifecycle pass (suggestions only).
    IdleCurator,
    /// Memory/knowledge nudges from chat feedback.
    MemoryNudge,
    /// The write-approval gate every proposal must pass.
    WriteApprovalGate,
}

impl HermesMechanism {
    /// All five stages, in loop order.
    pub const ALL: [Self; 5] = [
        Self::InUseImprovement,
        Self::PostTurnReview,
        Self::IdleCurator,
        Self::MemoryNudge,
        Self::WriteApprovalGate,
    ];

    /// Short stage name.
    pub fn stage_name(&self) -> &'static str {
        match self {
            Self::InUseImprovement => "in-use improvement",
            Self::PostTurnReview => "post-turn background review",
            Self::IdleCurator => "idle curator",
            Self::MemoryNudge => "memory nudge",
            Self::WriteApprovalGate => "write-approval gate",
        }
    }

    /// One-line description of what the stage does.
    pub fn description(&self) -> &'static str {
        match self {
            Self::InUseImprovement => {
                "captures rejections/denials/post-output edits into the correction journal"
            }
            Self::PostTurnReview => "replays the finished turn in a fork and proposes skill drafts",
            Self::IdleCurator => "classifies skill lifecycles and suggests stale/archive actions",
            Self::MemoryNudge => {
                "turns chat feedback into regenerable knowledge-suggestion updates"
            }
            Self::WriteApprovalGate => {
                "human approval + replay eval: the only path from draft to routable skill"
            }
        }
    }
}
