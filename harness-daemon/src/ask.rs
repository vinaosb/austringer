//! The permission-ask wire vocabulary shared by every transport:
//! answers, ask payloads, and resolutions. Extracted from `protocol`
//! to honor the per-file size budget; re-exported there so existing
//! imports are unchanged.

use serde::{Deserialize, Serialize};

use harness_providers::identity::ModelIdentity;

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
/// [`harness_providers::identity::FallbackRequest`] plus the ask
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
