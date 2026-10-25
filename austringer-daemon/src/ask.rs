//! The permission-ask wire vocabulary shared by every transport:
//! answers, ask payloads, and resolutions. Extracted from `protocol`
//! to honor the per-file size budget; re-exported there so existing
//! imports are unchanged.

use serde::{Deserialize, Serialize};

use austringer_providers::identity::ModelIdentity;

/// A client's answer to a permission ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Approve this one call.
    AllowOnce,
    /// Approve and remember for the session.
    AllowSession,
    /// Refuse.
    Deny,
}

/// How one ask resolved. Exactly one variant is ever produced per ask
/// (the N1 invariant enforced by [`crate::mediation`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// An answering client allowed it.
    Allowed {
        /// Which client answered.
        by: String,
    },
    /// An answering client refused it.
    Denied {
        /// Which client answered.
        by: String,
    },
    /// A client cancelled it.
    Cancelled {
        /// Which client cancelled it.
        by: String,
    },
    /// The synchronous deadline fired with no valid answer.
    Timeout,
}

/// A broadcast permission ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskPayload {
    /// Ask correlation id.
    pub ask_id: u64,
    /// Tool requesting approval.
    pub tool: String,
    /// Unredacted target for the approving human (never model-visible).
    pub target: Option<String>,
    /// Redacted, model-safe reason.
    pub reason: String,
    /// Synchronous deadline in milliseconds after ask start.
    pub deadline_ms: u64,
}

/// A broadcast model-fallback ask (the T9 identity contract: routing
/// proposes, the user disposes). Carries the shape of a
/// [`austringer_providers::identity::FallbackRequest`] plus the ask
/// correlation id and synchronous deadline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FallbackAskPayload {
    /// Ask correlation id.
    pub ask_id: u64,
    /// The identity being switched away from.
    pub current: ModelIdentity,
    /// The proposed replacement identity.
    pub proposed: ModelIdentity,
    /// Why the switch is proposed (surfaced for the decision).
    pub reason: String,
    /// Synchronous deadline in milliseconds after ask start; a
    /// timeout retries the SAME (current) model, never a substitution.
    pub deadline_ms: u64,
}

/// A client's decision on a pending plan-approval ask (todo 16):
/// approve the artifact as-is, reject it, or edit it — an edit is the
/// Antigravity plan-as-editable-artifact receipt; the edited artifact
/// re-gates writes and re-asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanDecision {
    /// Approve: plan mode ends and writes re-arm per the autonomy
    /// matrix.
    Approve,
    /// Reject: the plan is refused; plan mode STAYS active and writes
    /// stay gated.
    Reject,
    /// Edit: replace the artifact; the revision bumps, writes re-gate,
    /// and a fresh ask broadcasts.
    Edit {
        /// The edited plan artifact (markdown: task list +
        /// files-to-touch), bounded by
        /// [`crate::protocol::MAX_TEXT_FIELD_BYTES`].
        artifact: String,
    },
}

/// How one plan ask resolved. Exactly one variant is ever produced per
/// ask (the N1 invariant); a timeout NEVER resolves
/// [`PlanOutcome::Approved`] — no surface may auto-approve a plan (the
/// Antigravity v1.3.2 skip-permissions receipt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanOutcome {
    /// An answering client approved the plan.
    Approved {
        /// Which client answered.
        by: String,
    },
    /// An answering client rejected the plan (plan mode stays active).
    Rejected {
        /// Which client answered.
        by: String,
    },
    /// An answering client edited the artifact (the plan re-gates).
    Edited {
        /// Which client answered.
        by: String,
        /// The edited artifact.
        artifact: String,
    },
    /// A client cancelled the ask (the plan stays pending).
    Cancelled {
        /// Which client cancelled.
        by: String,
    },
    /// The synchronous deadline fired with no valid answer; the plan
    /// stays pending and writes stay gated.
    Timeout,
}

/// A broadcast plan-approval ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanAskPayload {
    /// Ask correlation id.
    pub ask_id: u64,
    /// The pending plan the artifact belongs to.
    pub plan_id: u64,
    /// Artifact revision (1 at entry, bumped per edit).
    pub revision: u64,
    /// The plan artifact awaiting approve/edit/reject.
    pub artifact: String,
    /// Synchronous deadline in milliseconds after ask start.
    pub deadline_ms: u64,
}
