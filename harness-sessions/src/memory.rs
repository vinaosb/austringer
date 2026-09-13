//! Curated memory notes (`/memory add`, `/recall`): typed storage with
//! an FTS5 index over titles and bodies.

use rusqlite::{Connection, OptionalExtension};

use crate::error::SessionError;
use crate::store::SessionStore;

/// A stored memory note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryNote {
    /// Stable id (rowid).
    pub id: i64,
    /// Originating session, if any.
    pub session_id: Option<String>,
    /// Short title.
    pub title: String,
    /// Body text.
    pub body: String,
    /// Creation time (clock ticks).
    pub created_at: u64,
    /// Last update time (drives temporal decay).
    pub updated_at: u64,
}

/// Input for [`SessionStore::memory_add`].
#[derive(Debug, Clone)]
pub struct NoteInput {
    /// Originating session, if any.
    pub session_id: Option<String>,
    /// Short title.
    pub title: String,
    /// Body text.
    pub body: String,
    /// Optional dense embedding for the vector half of hybrid recall.
    pub vector: Option<Vec<f32>>,
}

impl SessionStore {
    /// `/memory add`: persists a note, indexes its text, upserts its
    /// vector when one is attached and a vector index is set.
    ///
    /// # Errors
    /// Typed storage failure.
    pub fn memory_add(&self, note: NoteInput) -> Result<MemoryNote, SessionError> {
        let now = self.now();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction().map_err(SessionError::sqlite)?;
        tx.execute(
            "INSERT INTO memory_notes (session_id, created_at, updated_at, title, body)
             VALUES (?1, ?2, ?2, ?3, ?4)",
            rusqlite::params![note.session_id, now as i64, note.title, note.body],
        )
        .map_err(SessionError::sqlite)?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO memory_fts (note_id, title, body) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, note.title, note.body],
        )
        .map_err(SessionError::sqlite)?;
        tx.commit().map_err(SessionError::sqlite)?;
        drop(conn);
        if let Some(vector) = &note.vector
            && let Some(index) = self.vector_index()
        {
            index.upsert(id, vector)?;
        }
        Ok(MemoryNote {
            id,
            session_id: note.session_id,
            title: note.title,
            body: note.body,
            created_at: now,
            updated_at: now,
        })
    }

    /// Lists every note, oldest first.
    ///
    /// # Errors
    /// Typed storage failure.
    pub fn memory_notes(&self) -> Result<Vec<MemoryNote>, SessionError> {
        let conn = self.lock_conn()?;
        note_select(
            &conn,
            "SELECT id, session_id, title, body, created_at, updated_at FROM memory_notes ORDER BY id",
            [],
        )
    }

    /// Deletes a note and its index entries.
    ///
    /// # Errors
    /// Typed: not found, storage.
    pub fn memory_delete(&self, id: i64) -> Result<(), SessionError> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction().map_err(SessionError::sqlite)?;
        let changed = tx
            .execute("DELETE FROM memory_notes WHERE id = ?1", [id])
            .map_err(SessionError::sqlite)?;
        tx.execute("DELETE FROM memory_fts WHERE note_id = ?1", [id])
            .map_err(SessionError::sqlite)?;
        tx.commit().map_err(SessionError::sqlite)?;
        drop(conn);
        if changed == 0 {
            return Err(SessionError::NotFound { id: id.to_string() });
        }
        if let Some(index) = self.vector_index() {
            index.remove(id)?;
        }
        Ok(())
    }

    pub(crate) fn vector_index(&self) -> Option<std::sync::Arc<dyn crate::vector::VectorIndex>> {
        self.vectors.lock().ok().and_then(|guard| guard.clone())
    }

    /// Attaches (or clears) the vector half of hybrid recall.
    pub fn set_vector_index(&self, index: Option<std::sync::Arc<dyn crate::vector::VectorIndex>>) {
        if let Ok(mut slot) = self.vectors.lock() {
            *slot = index;
        }
    }

    pub(crate) fn recall_config(&self) -> crate::retrieval::RecallConfig {
        self.recall_config
            .lock()
            .map(|guard| *guard)
            .unwrap_or_default()
    }

    /// Overrides retrieval tuning (blend, decay, MMR, gate, depths).
    pub fn set_recall_config(&self, config: crate::retrieval::RecallConfig) {
        if let Ok(mut slot) = self.recall_config.lock() {
            *slot = config;
        }
    }
}

pub(crate) fn note_select(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<MemoryNote>, SessionError> {
    let mut stmt = conn.prepare(sql).map_err(SessionError::sqlite)?;
    let rows = stmt
        .query_map(params, |row| {
            Ok(MemoryNote {
                id: row.get(0)?,
                session_id: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get::<_, i64>(4)?.max(0) as u64,
                updated_at: row.get::<_, i64>(5)?.max(0) as u64,
            })
        })
        .map_err(SessionError::sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SessionError::sqlite)?;
    Ok(rows)
}

pub(crate) fn note_by_id(conn: &Connection, id: i64) -> Result<Option<MemoryNote>, SessionError> {
    conn.query_row(
        "SELECT id, session_id, title, body, created_at, updated_at FROM memory_notes WHERE id = ?1",
        [id],
        |row| {
            Ok(MemoryNote {
                id: row.get(0)?,
                session_id: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get::<_, i64>(4)?.max(0) as u64,
                updated_at: row.get::<_, i64>(5)?.max(0) as u64,
            })
        },
    )
    .optional()
    .map_err(SessionError::sqlite)
}
