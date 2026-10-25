//! Permissions-derived sandbox compile (plan todo 27): allowed
//! `read_url` grants compile into the todo-26 egress allowlist, so
//! ONE configuration source (the permission rules) drives both the
//! permission decision layer and the sandbox/egress enforcement
//! layer. Regex URL rules cannot compile — fail closed with a typed
//! error rather than silently diverging from enforcement.

use crate::egress::{EgressPolicy, HostPattern};
use crate::perm::grammar::{Action, Effect, Rule, TargetPattern};

/// Compiles the fetch-grant subset of `rules` into an
/// [`EgressPolicy`].
///
/// # Errors
/// [`CompileError`] for regex URL patterns (not representable as a
/// host allowlist entry) or malformed host patterns — the caller
/// refuses to run with rules the sandbox cannot enforce.
pub fn compile_egress(rules: &[Rule]) -> Result<EgressPolicy, CompileError> {
    let mut egress = EgressPolicy::none();
    for rule in rules {
        if rule.effect != Effect::Allow
            || !matches!(rule.action, Action::ReadUrl | Action::ExecuteUrl)
        {
            continue;
        }
        let TargetPattern::Prefix(pattern) = &rule.pattern else {
            return Err(CompileError::NotCompilable {
                origin: rule.origin.clone(),
            });
        };
        let host = HostPattern::parse(pattern).map_err(|_| CompileError::MalformedHost {
            origin: rule.origin.clone(),
        })?;
        let _ = egress.allow(host);
    }
    Ok(egress)
}

/// Egress compilation failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompileError {
    /// A URL rule used a regex pattern the allowlist cannot express.
    #[error("rule {origin:?} uses a regex url pattern the sandbox cannot enforce")]
    NotCompilable {
        /// The offending rule's origin label.
        origin: String,
    },
    /// A URL rule's host prefix was not a valid host pattern.
    #[error("rule {origin:?} names a malformed host")]
    MalformedHost {
        /// The offending rule's origin label.
        origin: String,
    },
}
