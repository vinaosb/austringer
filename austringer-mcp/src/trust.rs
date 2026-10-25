//! Trust store: approvals bind to the FULL entry fingerprint, never the
//! name alone (todo 29; Cursor MCPoison CVE-2025-54136). First use asks;
//! approval is recorded against the fingerprint; ANY identity change
//! (command, args, env, url, roots) mismatches the recorded hash and
//! re-asks. Auto-trust does not exist on this path — the T7 permission
//! seam gates the actual ask in the daemon.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::config::{Fingerprint, ServerEntry};

/// The trust verdict for one entry against one store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustDecision {
    /// The entry's fingerprint matches the recorded approval.
    Trusted,
    /// Never approved under this name: ask once (via the permission seam).
    NeedsApproval {
        /// The fingerprint approval would be recorded against.
        fingerprint: Fingerprint,
    },
    /// This name has an approval for a DIFFERENT fingerprint: the entry
    /// changed since approval; re-ask. Never inherit trust.
    ReApproval {
        /// The current entry's fingerprint.
        fingerprint: Fingerprint,
        /// The previously approved fingerprint.
        approved: Fingerprint,
    },
}

/// Storage for recorded approvals (name → last approved fingerprint).
/// Implementations persist as the daemon sees fit; this crate ships the
/// in-memory one and tests may script their own.
pub trait TrustStore: Send + Sync {
    /// The fingerprint currently approved for this server name, if any.
    fn approved(&self, name: &str) -> Option<Fingerprint>;
    /// Records (or replaces) the approval for one server name.
    fn record_approval(&self, name: &str, fingerprint: Fingerprint);
    /// Drops any approval for one server name.
    fn revoke(&self, name: &str);
}

/// In-memory [`TrustStore`] (tests, ephemeral sessions).
#[derive(Debug, Default)]
pub struct MemoryTrustStore {
    approvals: Mutex<HashMap<String, Fingerprint>>,
}

impl MemoryTrustStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TrustStore for MemoryTrustStore {
    fn approved(&self, name: &str) -> Option<Fingerprint> {
        self.approvals
            .lock()
            .map(|guard| guard.get(name).cloned())
            .ok()
            .flatten()
    }

    fn record_approval(&self, name: &str, fingerprint: Fingerprint) {
        if let Ok(mut guard) = self.approvals.lock() {
            guard.insert(name.to_owned(), fingerprint);
        }
    }

    fn revoke(&self, name: &str) {
        if let Ok(mut guard) = self.approvals.lock() {
            guard.remove(name);
        }
    }
}

impl ServerEntry {
    /// The trust verdict for this entry against a store.
    #[must_use]
    pub fn trust_decision(&self, store: &dyn TrustStore) -> TrustDecision {
        let fingerprint = self.fingerprint();
        match store.approved(&self.name) {
            None => TrustDecision::NeedsApproval { fingerprint },
            Some(approved) if approved == fingerprint => TrustDecision::Trusted,
            Some(approved) => TrustDecision::ReApproval {
                fingerprint,
                approved,
            },
        }
    }
}
