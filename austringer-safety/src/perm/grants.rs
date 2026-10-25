//! Stored-grant seam (plan todo 27): the T8 permission store adapts
//! onto [`GrantSource`]; grants are exact (action, target) approvals
//! with a typed origin (user `a` answer vs agent classifier decision).

use std::sync::Mutex;

use crate::perm::grammar::{Action, Target};

/// What produced a stored grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantOrigin {
    /// A user's persistent `a` answer (T8 seam).
    User,
    /// The agent classifier decided (T8 seam).
    Agent,
}

/// One stored grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    /// Who granted it.
    pub origin: GrantOrigin,
}

/// Stored-grant lookup seam (the T8 permission store adapts here).
pub trait GrantSource: Send + Sync {
    /// Looks up an exact (action, target) grant.
    fn lookup(&self, action: &Action, target: &Target) -> Option<Grant>;
}

/// In-memory grant store (tests + default engine).
#[derive(Debug, Default)]
pub struct MemoryGrants {
    grants: Mutex<Vec<(Action, Target, GrantOrigin)>>,
}

impl MemoryGrants {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one grant.
    pub fn grant(&self, action: Action, target: &Target, origin: GrantOrigin) {
        if let Ok(mut guard) = self.grants.lock() {
            guard.push((action, target.clone(), origin));
        }
    }
}

impl GrantSource for MemoryGrants {
    fn lookup(&self, action: &Action, target: &Target) -> Option<Grant> {
        self.grants.lock().ok().and_then(|guard| {
            guard
                .iter()
                .find(|(a, t, _)| a == action && t == target)
                .map(|(_, _, origin)| Grant { origin: *origin })
        })
    }
}
