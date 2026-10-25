//! Gated `/dream` consolidation: min-hours and min-sessions gates (injected
//! clock) must pass before any consolidation runs; the merge itself is
//! exact-duplicate collapse — LLM summarization is later wiring at the
//! daemon layer, behind the same gates.

use std::collections::BTreeMap;

use crate::error::SessionError;
use crate::store::SessionStore;
use crate::store::state_get;

/// Gates that must all hold before consolidation runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DreamGates {
    /// Minimum hours since the last consolidation (never-run passes).
    pub min_hours_since_last: u64,
    /// Minimum new sessions since the last consolidation.
    pub min_new_sessions: u32,
}

impl Default for DreamGates {
    fn default() -> Self {
        Self {
            min_hours_since_last: 24,
            min_new_sessions: 3,
        }
    }
}

/// Why the gates did not open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotYet {
    /// Hours still required.
    pub hours_remaining: u64,
    /// Sessions still required.
    pub sessions_remaining: u32,
}

/// The gated outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum DreamOutcome {
    /// Gates passed; duplicate notes were merged into their survivor.
    /// Each group is `(survivor, removed...)`.
    Consolidated {
        /// Duplicate groups: the first id survived, the rest were
        /// removed.
        merged: Vec<Vec<i64>>,
    },
    /// Gates closed; nothing ran.
    NotYet(Box<NotYet>),
}

/// Persisted gate inputs: `(last_consolidation_at, sessions_since)`.
/// `last_consolidation_at` is `None` when consolidation never ran.
///
/// # Errors
/// Typed storage failure.
pub fn dream_state(store: &SessionStore) -> Result<(Option<u64>, u64), SessionError> {
    let conn = store.lock_conn()?;
    let last = state_get(&conn, "last_dream_at")?;
    let sessions = state_get(&conn, "sessions_since_dream")?.max(0) as u64;
    Ok(((last > 0).then_some(last as u64), sessions))
}

impl SessionStore {
    /// Runs the gated consolidation. Returns [`DreamOutcome::NotYet`]
    /// with the exact remaining gates when closed — the daemon surfaces
    /// them; nothing is silently skipped.
    ///
    /// # Errors
    /// Typed storage failure.
    pub fn consolidate_memory(&self, gates: &DreamGates) -> Result<DreamOutcome, SessionError> {
        let now = self.now();
        let (last_dream_at, sessions_since) = dream_state(self)?;
        let hours_since = match last_dream_at {
            None => u64::MAX,
            Some(at) => now.saturating_sub(at) / 3600,
        };
        let hours_remaining = gates.min_hours_since_last.saturating_sub(hours_since);
        let sessions_remaining =
            u32::try_from(u64::from(gates.min_new_sessions).saturating_sub(sessions_since))
                .unwrap_or(u32::MAX);
        if hours_remaining > 0 || sessions_remaining > 0 {
            return Ok(DreamOutcome::NotYet(Box::new(NotYet {
                hours_remaining,
                sessions_remaining,
            })));
        }
        let merged = self.merge_duplicates()?;
        {
            let conn = self.lock_conn()?;
            conn.execute(
                "INSERT INTO store_state (key, value) VALUES ('last_dream_at', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![now as i64],
            )
            .map_err(SessionError::sqlite)?;
            conn.execute(
                "INSERT INTO store_state (key, value) VALUES ('sessions_since_dream', 0)
                 ON CONFLICT(key) DO UPDATE SET value = 0",
                [],
            )
            .map_err(SessionError::sqlite)?;
        }
        Ok(DreamOutcome::Consolidated { merged })
    }

    fn merge_duplicates(&self) -> Result<Vec<Vec<i64>>, SessionError> {
        let mut conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare("SELECT id, title, body FROM memory_notes ORDER BY id")
            .map_err(SessionError::sqlite)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(SessionError::sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::sqlite)?;
        drop(stmt);
        let mut groups: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for (id, title, body) in rows {
            let fingerprint = format!(
                "{}\u{1f}\u{1e}{}",
                title.trim().to_lowercase(),
                body.trim().to_lowercase()
            );
            groups.entry(fingerprint).or_default().push(id);
        }
        let merged: Vec<Vec<i64>> = groups.into_values().filter(|g| g.len() > 1).collect();
        let doomed: Vec<i64> = merged
            .iter()
            .flat_map(|group| group.iter().skip(1).copied())
            .collect();
        let tx = conn.transaction().map_err(SessionError::sqlite)?;
        for id in &doomed {
            tx.execute("DELETE FROM memory_notes WHERE id = ?1", [id])
                .map_err(SessionError::sqlite)?;
            tx.execute("DELETE FROM memory_fts WHERE note_id = ?1", [id])
                .map_err(SessionError::sqlite)?;
        }
        tx.commit().map_err(SessionError::sqlite)?;
        drop(conn);
        if let Some(index) = self.vector_index() {
            for id in &doomed {
                index.remove(*id)?;
            }
        }
        Ok(merged)
    }
}
