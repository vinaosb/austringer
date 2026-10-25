//! Invocation ledger: persisted per-job fire rows giving the runner
//! restart cursors and cap accounting. Caps are the enforcement; the
//! row cap only bounds the file size.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Ledger rows retained before the oldest are dropped.
const LEDGER_ROW_CAP: usize = 10_000;

/// One recorded fire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LedgerRow {
    /// Job that fired.
    pub(crate) job: String,
    /// Epoch minute of the fire.
    pub(crate) minute: u64,
}

/// Per-job invocation accounting over persisted rows.
#[derive(Default)]
pub(crate) struct InvocationLedger {
    rows: Vec<LedgerRow>,
}

impl InvocationLedger {
    /// Lifetime fire count for `job`.
    pub(crate) fn count_total(&self, job: &str) -> u64 {
        self.rows.iter().filter(|r| r.job == job).count() as u64
    }

    /// Fire count for `job` on UTC day `day`.
    pub(crate) fn count_day(&self, job: &str, day: u64) -> u64 {
        self.rows
            .iter()
            .filter(|r| r.job == job && r.minute / 1440 == day)
            .count() as u64
    }

    /// Highest fire minute for `job` (the restart cursor).
    pub(crate) fn cursor(&self, job: &str) -> Option<u64> {
        self.rows
            .iter()
            .filter(|r| r.job == job)
            .map(|r| r.minute)
            .max()
    }

    /// Record one fire, bounding total rows.
    pub(crate) fn record(&mut self, job: &str, minute: u64) {
        self.rows.push(LedgerRow {
            job: job.to_owned(),
            minute,
        });
        let excess = self.rows.len().saturating_sub(LEDGER_ROW_CAP);
        if excess > 0 {
            self.rows.drain(..excess);
        }
    }

    /// Persist to `{root}/scheduler-ledger.json`.
    ///
    /// # Errors
    /// serialization/IO failure detail.
    pub(crate) fn save(&self, root: &Path) -> Result<(), String> {
        let path = root.join("scheduler-ledger.json");
        let bytes = serde_json::to_vec(&self.rows).map_err(|e| e.to_string())?;
        std::fs::write(&path, bytes).map_err(|e| format!("{path:?}: {e}"))
    }

    /// Load (absent/corrupt file = empty ledger: cursors rebuild from
    /// the next fire).
    pub(crate) fn load(root: &Path) -> Self {
        let rows = std::fs::read(root.join("scheduler-ledger.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { rows }
    }
}
