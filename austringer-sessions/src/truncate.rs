//! `/rewind` log truncation: reset a session to a restore head in one
//! transaction (plan todo 5). Events, FTS rows, and turn-index entries
//! above the head are dropped together — search can never return
//! un-happened events — and the counters are recomputed so the next
//! append continues at head + 1.

use rusqlite::{Connection, OptionalExtension};

use austringer_core::event::Sequence;

use crate::error::SessionError;
use crate::session::SessionHandle;

impl SessionHandle<'_> {
    /// Truncates the log to `head` in one transaction: event rows, FTS
    /// rows, and turn-index entries above the head are deleted, the
    /// straddling turn is clipped, and `last_seq`/`event_count`/
    /// `turn_count`/`payload_bytes` reset to the head. The `/rewind`
    /// contract: the next append continues at `head + 1` and search
    /// never returns un-happened events.
    ///
    /// # Errors
    /// Typed: read-only handle, archived, head beyond the log, storage.
    pub fn truncate_to(&self, head: Sequence) -> Result<(), SessionError> {
        self.assert_writable()?;
        let mut guard = self.store.lock_conn()?;
        let tx = guard.transaction().map_err(SessionError::sqlite)?;
        truncate_in_tx(&tx, &self.meta.id, head, self.store.now())?;
        tx.commit().map_err(SessionError::sqlite)?;
        Ok(())
    }
}

/// One truncation inside the caller's transaction: the FTS rows above
/// the head are dropped in the SAME transaction as the events and the
/// turn index (an index rebuilt separately could observe un-happened
/// rows), the straddling turn is clipped, and the counters are
/// recomputed from the surviving rows.
fn truncate_in_tx(
    conn: &Connection,
    session_id: &str,
    head: Sequence,
    now: u64,
) -> Result<(), SessionError> {
    let last_seq: i64 = conn
        .query_row(
            "SELECT last_seq FROM sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(SessionError::sqlite)?
        .ok_or_else(|| SessionError::NotFound {
            id: session_id.to_owned(),
        })?;
    let last = u64::try_from(last_seq).unwrap_or(u64::MAX);
    if head.0 > last {
        return Err(SessionError::TruncateBeyondLog {
            id: session_id.to_owned(),
            at: head.0,
            last,
        });
    }
    let head_i = head.0 as i64;
    conn.execute(
        "DELETE FROM events WHERE session_id = ?1 AND seq > ?2",
        rusqlite::params![session_id, head_i],
    )
    .map_err(SessionError::sqlite)?;
    conn.execute(
        "DELETE FROM event_fts WHERE session_id = ?1 AND seq > ?2",
        rusqlite::params![session_id, head_i],
    )
    .map_err(SessionError::sqlite)?;
    conn.execute(
        "DELETE FROM turn_index WHERE session_id = ?1 AND start_seq > ?2",
        rusqlite::params![session_id, head_i],
    )
    .map_err(SessionError::sqlite)?;
    conn.execute(
        "UPDATE turn_index SET end_seq = ?2 WHERE session_id = ?1 AND end_seq > ?2",
        rusqlite::params![session_id, head_i],
    )
    .map_err(SessionError::sqlite)?;
    conn.execute(
        "UPDATE sessions SET last_seq = ?2,
         event_count = (SELECT COUNT(*) FROM events WHERE session_id = ?1),
         turn_count = (SELECT COUNT(*) FROM turn_index WHERE session_id = ?1),
         payload_bytes = (SELECT COALESCE(SUM(LENGTH(payload)), 0)
                          FROM events WHERE session_id = ?1),
         updated_at = ?3 WHERE id = ?1",
        rusqlite::params![session_id, head_i, now as i64],
    )
    .map_err(SessionError::sqlite)?;
    Ok(())
}
