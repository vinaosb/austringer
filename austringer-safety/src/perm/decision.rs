//! The engine's decision value (plan todo 27): effect, typed reason,
//! resolution path, and notice. Every layer that contributes records
//! itself in `layers`, so the user sees WHY — never just THAT.

use crate::perm::grammar::Effect;
use crate::perm::reasons::PermissionReason;

/// The decision: effect + typed reason + resolution path + notice.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    /// The outcome.
    pub effect: Effect,
    /// Whether an explicit policy decision (rule/grant/prefix)
    /// produced an allow.
    pub authoritative: bool,
    /// Machine-readable reason surfaced to the user.
    pub reason: PermissionReason,
    /// Layer trace, in evaluation order.
    pub layers: Vec<&'static str>,
    /// Optional user-facing notice (clamps, redactions).
    pub notice: Option<String>,
}

/// Builds a deny (never authoritative — that flag is about allows).
pub(crate) fn deny(reason: PermissionReason) -> Decision {
    Decision {
        effect: Effect::Deny,
        authoritative: false,
        reason,
        layers: Vec::new(),
        notice: None,
    }
}

/// Builds an allow.
pub(crate) fn allow(reason: PermissionReason, authoritative: bool) -> Decision {
    Decision {
        effect: Effect::Allow,
        authoritative,
        reason,
        layers: Vec::new(),
        notice: None,
    }
}

/// Builds an ask.
pub(crate) fn ask(reason: PermissionReason) -> Decision {
    Decision {
        effect: Effect::Ask,
        authoritative: false,
        reason,
        layers: Vec::new(),
        notice: None,
    }
}
