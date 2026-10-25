//! Turn index: derived, lazily-verified, rebuildable.
//!
//! A *turn* starts at each user event and extends to just before the
//! next one; events before any user belong to turn 1. The index is
//! maintained incrementally on append and can be rebuilt from the event
//! table's plaintext `kind` column without touching payloads (so
//! encrypted sessions rebuild without the key).

use rusqlite::{Connection, OptionalExtension};

use crate::error::SessionError;
use crate::meta::SessionMeta;

pub(crate) fn update_on_append(
    conn: &Connection,
    session_id: &str,
    kind: &str,
    seq: u64,
    turn_count: u32,
) -> Result<u32, SessionError> {
    if kind == "user" || turn_count == 0 {
        let turn = turn_count + 1;
        conn.execute(
            "INSERT INTO turn_index (session_id, turn, start_seq, end_seq) VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![session_id, turn, seq as i64],
        )
        .map_err(SessionError::sqlite)?;
        return Ok(turn);
    }
    conn.execute(
        "UPDATE turn_index SET end_seq = ?3 WHERE session_id = ?1 AND turn = ?2",
        rusqlite::params![session_id, turn_count, seq as i64],
    )
    .map_err(SessionError::sqlite)?;
    Ok(turn_count)
}

/// O(log n) consistency check between the index and the session
/// metadata: highest turn must equal the recorded turn count and its
/// `end_seq` must equal the last sequence. Interior row loss is caught
/// lazily by the range readers (row-count mismatch).
pub(crate) fn verify(conn: &Connection, meta: &SessionMeta) -> Result<(), SessionError> {
    if meta.turn_count == 0 {
        return Ok(());
    }
    let row: Option<(i64, Option<i64>)> = conn
        .query_row(
            "SELECT MAX(turn), (
                 SELECT end_seq FROM turn_index
                 WHERE session_id = ?1
                   AND turn = (SELECT MAX(turn) FROM turn_index WHERE session_id = ?1)
             ) FROM turn_index WHERE session_id = ?1",
            [meta.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(SessionError::sqlite)?;
    let Some((max_turn, end_seq)) = row else {
        return Err(SessionError::CorruptIndex {
            id: meta.id.clone(),
            detail: "metadata records turns but the index is empty".to_owned(),
        });
    };
    if u32::try_from(max_turn).unwrap_or(u32::MAX) != meta.turn_count {
        return Err(SessionError::CorruptIndex {
            id: meta.id.clone(),
            detail: format!(
                "index max turn {max_turn} != recorded turn count {}",
                meta.turn_count
            ),
        });
    }
    let end_seq = end_seq.ok_or_else(|| SessionError::CorruptIndex {
        id: meta.id.clone(),
        detail: "index max turn row is missing".to_owned(),
    })?;
    if u64::try_from(end_seq).unwrap_or(u64::MAX) != meta.last_seq {
        return Err(SessionError::CorruptIndex {
            id: meta.id.clone(),
            detail: format!(
                "index end seq {end_seq} != last event seq {}",
                meta.last_seq
            ),
        });
    }
    Ok(())
}

/// Rebuilds the index from the event table. Returns the turn count. A
/// damaged event log (sequence gaps) is a typed refusal — the index is
/// derived data and cannot be rebuilt from a broken source.
pub(crate) fn rebuild(
    conn: &Connection,
    session_id: &str,
    last_seq: u64,
) -> Result<u32, SessionError> {
    let mut stmt = conn
        .prepare("SELECT seq, kind FROM events WHERE session_id = ?1 ORDER BY seq")
        .map_err(SessionError::sqlite)?;
    let mut rows = stmt.query([session_id]).map_err(SessionError::sqlite)?;
    let mut expected = 1u64;
    let mut turns: Vec<(u32, u64, u64)> = Vec::new();
    while let Some(row) = rows.next().map_err(SessionError::sqlite)? {
        let seq: i64 = row.get(0).map_err(SessionError::sqlite)?;
        let kind: String = row.get(1).map_err(SessionError::sqlite)?;
        if u64::try_from(seq).unwrap_or(u64::MAX) != expected {
            return Err(SessionError::EventLogDamaged {
                id: session_id.to_owned(),
                detail: format!("expected sequence {expected}, found {seq}"),
            });
        }
        expected += 1;
        match turns.last_mut() {
            Some(current) if kind != "user" => current.2 = seq as u64,
            Some(_) | None => turns.push((turns.len() as u32 + 1, seq as u64, seq as u64)),
        }
    }
    if turns.last().is_some_and(|t| t.2 != last_seq) {
        return Err(SessionError::EventLogDamaged {
            id: session_id.to_owned(),
            detail: format!("event log ends before metadata last_seq {last_seq}"),
        });
    }
    conn.execute("DELETE FROM turn_index WHERE session_id = ?1", [session_id])
        .map_err(SessionError::sqlite)?;
    for (turn, start, end) in &turns {
        conn.execute(
            "INSERT INTO turn_index (session_id, turn, start_seq, end_seq) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![session_id, turn, *start as i64, *end as i64],
        )
        .map_err(SessionError::sqlite)?;
    }
    conn.execute(
        "UPDATE sessions SET turn_count = ?2 WHERE id = ?1",
        rusqlite::params![session_id, turns.len() as i64],
    )
    .map_err(SessionError::sqlite)?;
    Ok(turns.len() as u32)
}
