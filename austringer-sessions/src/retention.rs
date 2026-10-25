//! Retention: configurable archive/GC tiers behind one explicit `prune`
//! call. Nothing ever deletes without a caller-supplied policy; pinned
//! sessions are exempt; encrypted sessions are skipped with a typed
//! report entry (their ciphertext cannot round-trip the plaintext cold
//! file without a key — the caller exports them explicitly).

use std::path::PathBuf;

use rusqlite::OptionalExtension;

use crate::error::SessionError;
use crate::meta::{SessionMeta, load_meta_conn};
use crate::store::SessionStore;

/// The explicit, caller-supplied retention policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Archive sessions idle at least this many seconds (age tier).
    pub archive_after_secs: Option<u64>,
    /// Archive oldest-unpinned first until the store is under this many
    /// payload bytes (size tier).
    pub max_store_bytes: Option<u64>,
    /// Delete archived cold files after this many seconds (GC tier).
    pub gc_archived_after_secs: Option<u64>,
}

/// One archived session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedRecord {
    /// Session id.
    pub id: String,
    /// The cold JSONL file written before any row was deleted.
    pub path: PathBuf,
    /// Events preserved in the cold file.
    pub events: u64,
}

/// The prune report — every action and skip is itemized.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PruneReport {
    /// Sessions archived this run.
    pub archived: Vec<ArchivedRecord>,
    /// Archived files deleted by the GC tier.
    pub gc_removed: Vec<String>,
    /// Pinned sessions skipped by the age/size tiers.
    pub pinned_skipped: Vec<String>,
    /// Encrypted sessions skipped (need explicit keyed export).
    pub skipped_encrypted: Vec<String>,
}

impl SessionStore {
    /// The only deletion path in the store. Applies the policy tiers in
    /// order: GC of old archives, age tier, size tier. Never runs
    /// implicitly; an empty policy does nothing.
    ///
    /// # Errors
    /// Typed storage / IO failure (partial archives are impossible: the
    /// cold file is fully written and counted before any row deletion
    /// commits).
    pub fn prune(&self, policy: &RetentionPolicy) -> Result<PruneReport, SessionError> {
        let mut report = PruneReport::default();
        let now = self.now();
        if let Some(gc_after) = policy.gc_archived_after_secs {
            self.gc_archives(gc_after, now, &mut report)?;
        }
        if let Some(age) = policy.archive_after_secs {
            self.archive_tier(now.saturating_sub(age), &mut report)?;
        }
        if let Some(max_bytes) = policy.max_store_bytes {
            self.size_tier(max_bytes, &mut report)?;
        }
        Ok(report)
    }

    fn gc_archives(
        &self,
        gc_after: u64,
        now: u64,
        report: &mut PruneReport,
    ) -> Result<(), SessionError> {
        let cutoff = now.saturating_sub(gc_after);
        let candidates: Vec<(String, PathBuf)> = {
            let conn = self.lock_conn()?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, archived_at FROM sessions WHERE archived_at IS NOT NULL ORDER BY archived_at",
                )
                .map_err(SessionError::sqlite)?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u64,
                    ))
                })
                .map_err(SessionError::sqlite)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(SessionError::sqlite)?;
            rows.into_iter()
                .filter(|(_, archived_at)| *archived_at <= cutoff)
                .map(|(id, _)| {
                    let path = self.layout().archive_dir().join(format!("{id}.jsonl"));
                    (id, path)
                })
                .collect()
        };
        for (id, path) in candidates {
            std::fs::remove_file(&path).map_err(SessionError::io)?;
            let conn = self.lock_conn()?;
            conn.execute("DELETE FROM sessions WHERE id = ?1", [&id])
                .map_err(SessionError::sqlite)?;
            self.layout().remove_session(&id)?;
            report.gc_removed.push(id);
        }
        Ok(())
    }

    fn archive_tier(&self, cutoff: u64, report: &mut PruneReport) -> Result<(), SessionError> {
        let candidates: Vec<String> = {
            let conn = self.lock_conn()?;
            let mut stmt = conn
                .prepare(
                    "SELECT id FROM sessions WHERE archived_at IS NULL AND updated_at <= ?1 ORDER BY updated_at",
                )
                .map_err(SessionError::sqlite)?;
            stmt.query_map([cutoff as i64], |row| row.get::<_, String>(0))
                .map_err(SessionError::sqlite)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(SessionError::sqlite)?
        };
        for id in candidates {
            self.archive_one(&id, report)?;
        }
        Ok(())
    }

    fn size_tier(&self, max_bytes: u64, report: &mut PruneReport) -> Result<(), SessionError> {
        loop {
            let next = {
                let conn = self.lock_conn()?;
                let total: i64 = conn
                    .query_row(
                        "SELECT COALESCE(SUM(payload_bytes), 0) FROM sessions WHERE archived_at IS NULL",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(SessionError::sqlite)?;
                if total.max(0) as u64 <= max_bytes {
                    return Ok(());
                }
                conn.query_row(
                    "SELECT id FROM sessions
                     WHERE archived_at IS NULL AND pinned = 0 AND encrypted = 0
                     ORDER BY updated_at ASC LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(SessionError::sqlite)?
            };
            let Some(id) = next else {
                return Ok(());
            };
            self.archive_one(&id, report)?;
        }
    }

    fn archive_one(&self, id: &str, report: &mut PruneReport) -> Result<(), SessionError> {
        let meta = self.load_meta(id)?;
        if meta.pinned {
            report.pinned_skipped.push(id.to_owned());
            return Ok(());
        }
        if meta.encrypted {
            report.skipped_encrypted.push(id.to_owned());
            return Ok(());
        }
        let handle = self.open_session_mode(id, crate::session::OpenMode::ReadOnly)?;
        let archive_dir = self.layout().ensure_archive()?;
        let path = archive_dir.join(format!("{id}.jsonl"));
        let written = handle.export_jsonl_to(&path)?;
        if written != meta.event_count {
            std::fs::remove_file(&path).map_err(SessionError::io)?;
            return Err(SessionError::Corrupt {
                detail: format!(
                    "archive aborted: exported {written} events but metadata counts {}",
                    meta.event_count
                ),
            });
        }
        handle.release()?;
        drop(handle);
        let now = self.now();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction().map_err(SessionError::sqlite)?;
        tx.execute("DELETE FROM events WHERE session_id = ?1", [id])
            .map_err(SessionError::sqlite)?;
        tx.execute("DELETE FROM turn_index WHERE session_id = ?1", [id])
            .map_err(SessionError::sqlite)?;
        tx.execute("DELETE FROM event_fts WHERE session_id = ?1", [id])
            .map_err(SessionError::sqlite)?;
        tx.execute("DELETE FROM leases WHERE session_id = ?1", [id])
            .map_err(SessionError::sqlite)?;
        tx.execute(
            "UPDATE sessions SET archived_at = ?2, last_seq = 0, event_count = 0,
             turn_count = 0, payload_bytes = 0 WHERE id = ?1",
            rusqlite::params![id, now as i64],
        )
        .map_err(SessionError::sqlite)?;
        tx.commit().map_err(SessionError::sqlite)?;
        report.archived.push(ArchivedRecord {
            id: id.to_owned(),
            path,
            events: written,
        });
        Ok(())
    }
}

/// Reloads metadata for an archived session from its tombstone row.
///
/// # Errors
/// Typed: not found, storage.
pub fn archived_meta(store: &SessionStore, id: &str) -> Result<SessionMeta, SessionError> {
    let conn = store.lock_conn()?;
    load_meta_conn(&conn, id)
}
