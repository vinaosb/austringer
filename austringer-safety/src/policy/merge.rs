//! Policy merge + session-pin lifecycle (plan todo 26).
//!
//! Layered policies (org → automation → session) may only TIGHTEN
//! (Devin security-profile intersection): protected paths union,
//! writable roots and egress allowlists intersect, scrub lists union.
//! A pinned profile cannot be loosened mid-session.

use crate::policy::decl::{PinnedProfile, SandboxPolicy};
use std::path::PathBuf;

/// Merge two policy layers; deny always wins.
#[must_use]
pub fn merge_policies(base: &SandboxPolicy, overlay: &SandboxPolicy) -> SandboxPolicy {
    SandboxPolicy {
        tier: stricter_tier(base.tier, overlay.tier),
        writable_roots: intersect_roots(&base.writable_roots, &overlay.writable_roots),
        readable_roots: intersect_roots(&base.readable_roots, &overlay.readable_roots),
        protected_paths: union_unique(&base.protected_paths, &overlay.protected_paths),
        deny_globs: union_unique(&base.deny_globs, &overlay.deny_globs),
        egress: base.egress.intersect(&overlay.egress),
        env_scrub: base.env_scrub.union(&overlay.env_scrub),
    }
}

/// Attempt to replace a pinned profile mid-session: typed rejection.
///
/// # Errors
/// [`PinError::ProfilePinned`] — the profile is fixed for the session
/// lifetime; changes require a new session.
pub fn attempt_change(
    pinned: &PinnedProfile,
    _next: &SandboxPolicy,
) -> Result<SandboxPolicy, PinError> {
    Err(PinError::ProfilePinned {
        session_id: pinned.session_id.clone(),
    })
}

/// Restore a pinned profile from its serialized checkpoint form.
///
/// # Errors
/// [`PinRestoreError::Corrupted`] when the payload is not a valid pin.
pub fn restore(serialized: &str) -> Result<PinnedProfile, PinRestoreError> {
    serde_json::from_str(serialized).map_err(|_| PinRestoreError::Corrupted {
        detail: "pinned profile payload is not valid JSON".into(),
    })
}

/// Pinned-profile violations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PinError {
    /// The session already pinned a profile; mid-session change refused.
    #[error("session {session_id} has a pinned profile; refusing mid-session change")]
    ProfilePinned {
        /// The pinned session id.
        session_id: String,
    },
}

/// Restore failures.
#[derive(Debug, thiserror::Error)]
pub enum PinRestoreError {
    /// The serialized pin could not be parsed back.
    #[error("corrupted pinned profile: {detail}")]
    Corrupted {
        /// Why the payload was rejected.
        detail: String,
    },
}

fn stricter_tier(
    base: crate::policy::decl::SandboxTier,
    overlay: crate::policy::decl::SandboxTier,
) -> crate::policy::decl::SandboxTier {
    use crate::policy::decl::SandboxTier::{Container, Readonly, Worktree};
    match (base, overlay) {
        (Container, other) | (other, Container) => {
            // Container is a different isolation class, not a strict
            // tightening; carry the non-container side's tighter plain tier.
            let plain = if matches!(other, Container) {
                Worktree
            } else {
                other
            };
            if matches!(plain, Readonly) {
                Readonly
            } else {
                Worktree
            }
        }
        (Readonly, _) | (_, Readonly) => Readonly,
        (Worktree, Worktree) => Worktree,
    }
}

fn intersect_roots(base: &[PathBuf], overlay: &[PathBuf]) -> Vec<PathBuf> {
    base.iter()
        .filter(|p| overlay.contains(p))
        .cloned()
        .collect()
}

fn union_unique<T: PartialEq + Clone>(base: &[T], overlay: &[T]) -> Vec<T> {
    let mut merged = base.to_vec();
    for item in overlay {
        if !merged.contains(item) {
            merged.push(item.clone());
        }
    }
    merged
}
