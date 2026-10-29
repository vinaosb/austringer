//! Run history: one append-only JSONL row per scheduled-run lifecycle
//! event (kickoff, completion) under `{data}/runs.jsonl`, linked into
//! the session store by `session_id`. Torn tails are skipped on load,
//! never fatal.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::automations::NetworkPolicy;

/// One scheduled-run lifecycle row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Owning automation.
    pub automation_id: String,
    /// Session the run lives in.
    pub session_id: String,
    /// Cron minute the run fired on.
    pub fired_at_minute: u64,
    /// Lifecycle step (`started`, or the turn outcome on completion).
    pub outcome: String,
    /// Gate verdict on completion rows.
    pub verdict: Option<String>,
    /// Effective (intersected) network policy of the run.
    pub network: NetworkPolicy,
}

/// History failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunHistoryError {
    /// I/O failure.
    #[error("run history io: {0}")]
    Io(String),
}

/// Default rotation cap for `runs.jsonl` (16 MiB); matches
/// [`crate::config::DaemonConfig::runs_max_bytes`].
pub const DEFAULT_RUNS_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Append-only JSONL run history.
pub struct RunHistory {
    path: PathBuf,
    max_bytes: u64,
}

impl RunHistory {
    /// History at `{root}/runs.jsonl` (created on first append) with
    /// the default rotation cap ([`DEFAULT_RUNS_MAX_BYTES`]).
    ///
    /// # Errors
    /// none at open; appends/list surface failures.
    pub fn open(root: &Path) -> Result<Self, RunHistoryError> {
        Self::open_capped(root, DEFAULT_RUNS_MAX_BYTES)
    }

    /// History with an explicit rotation cap; `0` disables rotation
    /// (unlimited growth, the pre-rotation behavior).
    ///
    /// # Errors
    /// none at open; appends/list surface failures.
    pub fn open_capped(root: &Path, max_bytes: u64) -> Result<Self, RunHistoryError> {
        Ok(Self {
            path: root.join("runs.jsonl"),
            max_bytes,
        })
    }

    /// Append one row. Size-capped rotation at a record boundary: when
    /// the current file already exceeds `max_bytes`, it rotates to a
    /// single forensic `.1` generation (replacing any prior one)
    /// BEFORE this row is written. `list` reads the current file only,
    /// so `.1` is forensic-only and correct: the LATEST rows always
    /// live in current, only the OLDEST history is ever dropped.
    ///
    /// # Errors
    /// I/O failure.
    pub fn append(&self, record: &RunRecord) -> Result<(), RunHistoryError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| RunHistoryError::Io(format!("{:?}: {e}", parent)))?;
        }
        if crate::rotation::exceeds_cap(&self.path, self.max_bytes) {
            crate::rotation::rotate(&self.path)
                .map_err(|e| RunHistoryError::Io(format!("{:?}: {e}", self.path)))?;
        }
        let mut line =
            serde_json::to_vec(record).map_err(|e| RunHistoryError::Io(e.to_string()))?;
        line.push(b'\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut file| {
                use std::io::Write;
                file.write_all(&line)
            })
            .map_err(|e| RunHistoryError::Io(format!("{:?}: {e}", self.path)))
    }

    /// All readable rows (torn tail skipped).
    ///
    /// # Errors
    /// I/O failure.
    pub fn list(&self) -> Result<Vec<RunRecord>, RunHistoryError> {
        let raw = std::fs::read_to_string(&self.path)
            .map_err(|e| RunHistoryError::Io(format!("{:?}: {e}", self.path)))?;
        Ok(raw
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }

    /// Rows for one automation.
    ///
    /// # Errors
    /// I/O failure.
    pub fn for_automation(&self, automation: &str) -> Result<Vec<RunRecord>, RunHistoryError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|r| r.automation_id == automation)
            .collect())
    }

    /// Most recent row for one session (completion-row recovery).
    ///
    /// # Errors
    /// I/O failure.
    pub fn latest_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<RunRecord>, RunHistoryError> {
        Ok(self
            .list()?
            .iter()
            .rev()
            .find(|r| r.session_id == session_id)
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automations::NetworkPolicy;

    fn record(automation: &str, session: &str, outcome: &str) -> RunRecord {
        RunRecord {
            automation_id: automation.into(),
            session_id: session.into(),
            fired_at_minute: 600,
            outcome: outcome.into(),
            verdict: None,
            network: NetworkPolicy::Online,
        }
    }

    #[test]
    fn append_then_list_roundtrips() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let history = RunHistory::open(dir.path())?;
        history.append(&record("nightly", "auto-nightly-600", "started"))?;
        history.append(&record("nightly", "auto-nightly-600", "task_completed"))?;
        let rows = history.list()?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].outcome, "started");
        assert_eq!(rows[1].session_id, "auto-nightly-600");
        assert!(dir.path().join("runs.jsonl").exists());
        Ok(())
    }

    #[test]
    fn torn_tail_is_skipped_not_fatal() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let history = RunHistory::open(dir.path())?;
        history.append(&record("nightly", "s1", "started"))?;
        let path = dir.path().join("runs.jsonl");
        let mut file = std::fs::OpenOptions::new().append(true).open(&path)?;
        std::io::Write::write_all(&mut file, b"{\"automation_id\":\"ni")?;
        drop(file);
        let rows = RunHistory::open(dir.path())?.list()?;
        assert_eq!(rows.len(), 1);
        Ok(())
    }

    #[test]
    fn for_automation_filters_and_verdict_roundtrips() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let history = RunHistory::open(dir.path())?;
        let mut done = record("nightly", "s1", "confirmed");
        done.verdict = Some("confirmed".into());
        history.append(&record("other", "s2", "started"))?;
        history.append(&done)?;
        let nightly = history.for_automation("nightly")?;
        assert_eq!(nightly.len(), 1);
        assert_eq!(nightly[0].verdict.as_deref(), Some("confirmed"));
        Ok(())
    }
}
