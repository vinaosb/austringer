//! Lazy paging: turns and event ranges page in through primary-key
//! scans; interior index corruption is a typed error, never silent
//! truncation.

use rusqlite::Connection;

use austringer_core::event::{RecordedEvent, Sequence};

use crate::error::SessionError;
use crate::index;
use crate::session::SessionHandle;

/// A paged-in turn with its events.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnPage {
    /// 1-based turn number.
    pub turn: u32,
    /// The turn's events in sequence order.
    pub events: Vec<RecordedEvent>,
}

pub(crate) fn no_such_turn(id: &str, turn: u32, turns: u32) -> SessionError {
    SessionError::NoSuchTurn {
        id: id.to_owned(),
        turn,
        turns,
    }
}

impl SessionHandle<'_> {
    /// Pages in turns `from..=to`. Asserts the row count so interior
    /// index corruption is a typed [`SessionError::CorruptIndex`], not
    /// silent data loss.
    ///
    /// # Errors
    /// Typed: no such turn, corrupt index, decrypt, storage.
    pub fn turns_range(&self, from: u32, to: u32) -> Result<Vec<TurnPage>, SessionError> {
        if from == 0 || to < from || to > self.meta.turn_count {
            return Err(no_such_turn(&self.meta.id, from, self.meta.turn_count));
        }
        let conn = self.store.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT turn, start_seq, end_seq FROM turn_index
                 WHERE session_id = ?1 AND turn BETWEEN ?2 AND ?3 ORDER BY turn",
            )
            .map_err(SessionError::sqlite)?;
        let bounds = stmt
            .query_map(rusqlite::params![self.meta.id, from, to], |row| {
                Ok((
                    u32::try_from(row.get::<_, i64>(0)?).unwrap_or(u32::MAX),
                    row.get::<_, i64>(1)?.max(0) as u64,
                    row.get::<_, i64>(2)?.max(0) as u64,
                ))
            })
            .map_err(SessionError::sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::sqlite)?;
        if bounds.len() as u32 != to - from + 1 {
            return Err(SessionError::CorruptIndex {
                id: self.meta.id.clone(),
                detail: format!(
                    "index has {} rows for turns {from}-{to}; expected {}",
                    bounds.len(),
                    to - from + 1
                ),
            });
        }
        drop(stmt);
        bounds
            .into_iter()
            .map(|(turn, start, end)| {
                let events = read_range(self, &conn, start, end)?;
                Ok(TurnPage { turn, events })
            })
            .collect()
    }

    /// Pages in a single turn.
    ///
    /// # Errors
    /// Typed: no such turn, corrupt index, decrypt, storage.
    pub fn turn(&self, turn: u32) -> Result<TurnPage, SessionError> {
        let mut pages = self.turns_range(turn, turn)?;
        pages
            .pop()
            .ok_or_else(|| no_such_turn(&self.meta.id, turn, self.meta.turn_count))
    }

    /// Reads raw events by sequence range (inclusive).
    ///
    /// # Errors
    /// Typed: decrypt, storage.
    pub fn events_range(
        &self,
        from: Sequence,
        to: Sequence,
    ) -> Result<Vec<RecordedEvent>, SessionError> {
        if to.0 < from.0 || from.0 == 0 {
            return Ok(Vec::new());
        }
        let conn = self.store.lock_conn()?;
        read_range(self, &conn, from.0, to.0)
    }

    /// Rebuilds the derived turn index from the event table.
    ///
    /// # Errors
    /// Typed: damaged event log (gaps) is refused; storage.
    pub fn repair_index(&self) -> Result<u32, SessionError> {
        let conn = self.store.lock_conn()?;
        index::rebuild(&conn, &self.meta.id, self.meta.last_seq)
    }
}

pub(crate) fn decrypt_payload(
    handle: &SessionHandle<'_>,
    payload: &[u8],
) -> Result<Vec<u8>, SessionError> {
    match handle.key.as_ref() {
        Some(key) => key.open(handle.meta.id.as_bytes(), payload),
        None => Ok(payload.to_vec()),
    }
}

fn read_range(
    handle: &SessionHandle<'_>,
    conn: &Connection,
    from: u64,
    to: u64,
) -> Result<Vec<RecordedEvent>, SessionError> {
    let mut stmt = conn
        .prepare(
            "SELECT payload FROM events WHERE session_id = ?1 AND seq BETWEEN ?2 AND ?3 ORDER BY seq",
        )
        .map_err(SessionError::sqlite)?;
    let rows = stmt
        .query_map(
            rusqlite::params![handle.meta.id, from as i64, to as i64],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .map_err(SessionError::sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SessionError::sqlite)?;
    rows.into_iter()
        .map(|payload| {
            let bytes = decrypt_payload(handle, &payload)?;
            serde_json::from_slice::<RecordedEvent>(&bytes).map_err(|error| SessionError::Corrupt {
                detail: format!("event payload malformed: {error}"),
            })
        })
        .collect()
}
