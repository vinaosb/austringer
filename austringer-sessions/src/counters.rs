//! Session counters: the append-sequence state threaded through
//! transactional bulk writes so the `sessions` row is read once per
//! transaction instead of once per event.

use rusqlite::{Connection, OptionalExtension};

use crate::error::SessionError;

/// The `sessions`-row state an append depends on. Inside one
/// transaction SQLite is single-writer, so a bulk append loads these
/// once and lets [`crate::session::append_in_tx`] advance them in
/// memory instead of re-reading the row the previous append just wrote.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionCounters {
    /// Last persisted sequence (storage representation).
    pub(crate) last_seq: i64,
    /// User-anchored turn count.
    pub(crate) turn_count: u32,
    /// Whether payloads are sealed under a per-session key.
    pub(crate) encrypted: bool,
}

impl SessionCounters {
    /// Loads the counters of one session row. Typed [`SessionError::NotFound`]
    /// when the row is absent — bulk callers surface this on their first
    /// read, before any event is written.
    ///
    /// # Errors
    /// Typed: not found, storage.
    pub(crate) fn load(conn: &Connection, session_id: &str) -> Result<Self, SessionError> {
        conn.prepare_cached("SELECT last_seq, turn_count, encrypted FROM sessions WHERE id = ?1")
            .map_err(SessionError::sqlite)?
            .query_row([session_id], |row| {
                Ok(Self {
                    last_seq: row.get(0)?,
                    turn_count: u32::try_from(row.get::<_, i64>(1)?).unwrap_or(u32::MAX),
                    encrypted: row.get::<_, i64>(2)? != 0,
                })
            })
            .optional()
            .map_err(SessionError::sqlite)?
            .ok_or_else(|| SessionError::NotFound {
                id: session_id.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use austringer_core::event::{ContentBlock, Event, Sequence};

    use crate::lease::LeaseOwner;
    use crate::store::SessionStore;

    thread_local! {
        static COUNTERS_SELECTS: Cell<u64> = const { Cell::new(0) };
    }

    fn count_counters_selects(event: rusqlite::trace::TraceEvent<'_>) {
        if let rusqlite::trace::TraceEvent::Stmt(_, sql) = event
            && sql.contains("last_seq, turn_count, encrypted")
        {
            COUNTERS_SELECTS.with(|cell| cell.set(cell.get() + 1));
        }
    }

    /// Inside one transaction SQLite is single-writer: per-event
    /// re-reads of the counters row return exactly what the previous
    /// iteration wrote. A bulk append must read it once per batch.
    #[test]
    fn append_batch_reads_counters_row_once() {
        let root = tempfile::tempdir().unwrap();
        let store = SessionStore::open(root.path()).unwrap();
        store.create("bulk", "").unwrap();
        let events: Vec<Event> = (0..50)
            .map(|i| Event::User {
                blocks: vec![ContentBlock::Text {
                    text: format!("bulk event {i}"),
                }],
            })
            .collect();
        {
            let handle = store
                .open_session_rw("bulk", &LeaseOwner::new("w"))
                .unwrap();
            {
                let conn = store.lock_conn().unwrap();
                conn.trace_v2(
                    rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
                    Some(count_counters_selects),
                );
            }
            COUNTERS_SELECTS.with(|cell| cell.set(0));
            let last = handle.append_batch(&events).unwrap();
            assert_eq!(last, Sequence(50));
        }
        let reads = COUNTERS_SELECTS.with(|cell| cell.get());
        assert_eq!(
            reads, 1,
            "append_batch of 50 events must read the counters row exactly once, got {reads}"
        );
    }
}
