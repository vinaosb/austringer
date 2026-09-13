//! Audit seam (plan todo 27): an append-only record of EVERY
//! permission decision. The todo-4 event log is core-side and stays
//! there; this seam is the permission engine's own typed trail,
//! sinkable into any store the embedder chooses.

use std::sync::Mutex;

use crate::perm::grammar::{Action, Effect};
use crate::perm::reasons::PermissionReason;

/// One audited decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// The action that was judged.
    pub action: Action,
    /// Redacted target summary (secret paths masked; URLs carry
    /// scheme://host only).
    pub target_summary: String,
    /// The decision's effect.
    pub effect: Effect,
    /// The machine-readable reason.
    pub reason: PermissionReason,
    /// Whether the engine ran in bypass/yolo mode.
    pub bypass: bool,
    /// Wall-clock milliseconds of the decision.
    pub at_ms: u64,
}

/// Append-only audit destination. Implementations must never mutate
/// or drop prior entries.
pub trait AuditSink: Send + Sync {
    /// Records one entry.
    fn record(&self, entry: AuditEntry);
}

/// In-memory sink (tests, daemons buffering before flush).
#[derive(Debug, Default)]
pub struct MemoryAuditSink {
    entries: Mutex<Vec<AuditEntry>>,
}

impl MemoryAuditSink {
    /// An empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of every recorded entry, in order.
    #[must_use]
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl AuditSink for MemoryAuditSink {
    fn record(&self, entry: AuditEntry) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.push(entry);
        }
    }
}
