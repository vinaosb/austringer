//! Hook config trust review + hash pinning (Codex model): first use of
//! a hook requires review (the hook stays inert), approval pins the
//! config's content hash, and a later content change is a typed
//! mismatch that re-inerts the hook until re-approval. The registry is
//! serde-serializable so pins survive restarts (the session store or
//! daemon persists the snapshot).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The registry's verdict for one (name, hash) pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustCheck {
    /// No pin exists: a human must review before the hook ever runs.
    PendingReview,
    /// The content hash matches the pin.
    Approved,
    /// A pin exists but the content changed: re-review required.
    Modified {
        /// The hash the config now carries.
        current: String,
    },
}

/// Approved-config pins keyed by hook name. Serialize/Deserialize round
/// trips are the restart path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRegistry {
    entries: BTreeMap<String, String>,
}

impl TrustRegistry {
    /// An empty registry (everything pending review).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Classifies one config against its pin.
    #[must_use]
    pub fn classify(&self, name: &str, hash: &str) -> TrustCheck {
        match self.entries.get(name) {
            Some(pinned) if pinned == hash => TrustCheck::Approved,
            Some(_) => TrustCheck::Modified {
                current: hash.to_owned(),
            },
            None => TrustCheck::PendingReview,
        }
    }

    /// Pins (or re-pins) `hash` for `name` — the human approval action.
    pub fn approve(&mut self, name: &str, hash: String) {
        self.entries.insert(name.to_owned(), hash);
    }

    /// Drops the pin for `name` (back to pending review).
    pub fn revoke(&mut self, name: &str) {
        self.entries.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_walks_the_review_lifecycle() {
        let mut registry = TrustRegistry::new();
        assert_eq!(registry.classify("h", "a"), TrustCheck::PendingReview);
        registry.approve("h", "a".to_owned());
        assert_eq!(registry.classify("h", "a"), TrustCheck::Approved);
        assert!(matches!(
            registry.classify("h", "b"),
            TrustCheck::Modified { .. }
        ));
        registry.approve("h", "b".to_owned());
        assert_eq!(registry.classify("h", "b"), TrustCheck::Approved);
        registry.revoke("h");
        assert_eq!(registry.classify("h", "b"), TrustCheck::PendingReview);
    }

    #[test]
    fn registry_round_trips_through_serialization() {
        let mut registry = TrustRegistry::new();
        registry.approve("h", "pin".to_owned());
        let text = serde_json::to_string(&registry).expect("serialize");
        let restored: TrustRegistry = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(restored, registry);
    }
}
