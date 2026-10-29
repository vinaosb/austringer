//! Lazy session handles: open reads metadata and index bounds only;
//! appends are transactional and leased.

use rusqlite::Connection;

use austringer_core::event::{Event, RecordedEvent, Sequence};

use crate::counters::SessionCounters;
use crate::crypto::SessionKey;
use crate::error::SessionError;
use crate::index;
use crate::keystore::unlock_session_key;
use crate::lease::{self, Acquired, LeaseOwner, TakeoverReport};
use crate::meta::{SessionMeta, extract_text};
use crate::store::SessionStore;

/// How a session is opened.
#[derive(Debug, Clone)]
pub enum OpenMode {
    /// Read-only view: no lease interaction, appends typed-rejected.
    ReadOnly,
    /// Read-write: acquires (or renews) the writer lease for `owner`.
    ReadWrite(LeaseOwner),
}

/// An open session. Opening is O(1) in event data: the metadata row and
/// an O(log n) index check. Payloads are read only by the paging
/// methods. `meta()` is the snapshot from open; reopen for fresh
/// counters after other handles wrote.
pub struct SessionHandle<'a> {
    pub(crate) store: &'a SessionStore,
    pub(crate) meta: SessionMeta,
    pub(crate) key: Option<SessionKey>,
    pub(crate) lease_owner: Option<LeaseOwner>,
    takeover: Option<TakeoverReport>,
}

impl std::fmt::Debug for SessionHandle<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHandle")
            .field("id", &self.meta.id)
            .field("last_seq", &self.meta.last_seq)
            .field("writable", &self.lease_owner.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a> SessionHandle<'a> {
    pub(crate) fn open(
        store: &'a SessionStore,
        id: &str,
        mode: &OpenMode,
        passphrase: Option<&str>,
    ) -> Result<Self, SessionError> {
        let conn = store.lock_conn()?;
        let meta = crate::meta::load_meta_conn(&conn, id)?;
        index::verify(&conn, &meta)?;
        let key = if meta.encrypted {
            let Some(passphrase) = passphrase else {
                return Err(SessionError::MissingPassphrase { id: id.to_owned() });
            };
            Some(unlock(&conn, &meta, passphrase)?)
        } else {
            None
        };
        let mut lease_owner = None;
        let mut takeover = None;
        if let OpenMode::ReadWrite(owner) = mode {
            if meta.archived_at.is_some() {
                return Err(SessionError::Archived { id: id.to_owned() });
            }
            if let Acquired::Takeover(report) =
                lease::acquire(&conn, id, owner, store.probe().as_ref(), store.now())?
            {
                takeover = Some(report);
            }
            lease_owner = Some(owner.clone());
        }
        Ok(Self {
            store,
            meta,
            key,
            lease_owner,
            takeover,
        })
    }

    /// Session metadata (the snapshot read at open).
    #[must_use]
    pub fn meta(&self) -> &SessionMeta {
        &self.meta
    }

    /// The takeover report when this open replaced a stale lease.
    #[must_use]
    pub fn takeover(&self) -> Option<&TakeoverReport> {
        self.takeover.as_ref()
    }

    /// Last appended sequence (as of open).
    #[must_use]
    pub fn last_sequence(&self) -> Sequence {
        Sequence(self.meta.last_seq)
    }

    pub(crate) fn assert_writable(&self) -> Result<(), SessionError> {
        match &self.lease_owner {
            None => Err(SessionError::NotWritable {
                id: self.meta.id.clone(),
            }),
            Some(_) if self.meta.archived_at.is_some() => Err(SessionError::Archived {
                id: self.meta.id.clone(),
            }),
            Some(_) => Ok(()),
        }
    }

    /// Appends one event transactionally (event row, turn index,
    /// counters, FTS — all-or-nothing), continuing the sequence.
    ///
    /// # Errors
    /// Typed: read-only handle, archived, encrypt, storage.
    pub fn append(&self, event: Event) -> Result<Sequence, SessionError> {
        self.assert_writable()?;
        let mut guard = self.store.lock_conn()?;
        let tx = guard.transaction().map_err(SessionError::sqlite)?;
        let mut counters = SessionCounters::load(&tx, &self.meta.id)?;
        let seq = append_in_tx(
            &tx,
            &self.meta.id,
            self.key.as_ref(),
            &event,
            self.store.now(),
            &mut counters,
        )?;
        tx.commit().map_err(SessionError::sqlite)?;
        Ok(seq)
    }

    /// Appends many events in one transaction; returns the last sequence.
    /// Reads the counters row once for the whole batch.
    ///
    /// # Errors
    /// Same typed conditions as [`Self::append`].
    pub fn append_batch(&self, events: &[Event]) -> Result<Sequence, SessionError> {
        self.assert_writable()?;
        let mut guard = self.store.lock_conn()?;
        let tx = guard.transaction().map_err(SessionError::sqlite)?;
        let mut counters = SessionCounters::load(&tx, &self.meta.id)?;
        let mut last = Sequence(0);
        for event in events {
            last = append_in_tx(
                &tx,
                &self.meta.id,
                self.key.as_ref(),
                event,
                self.store.now(),
                &mut counters,
            )?;
        }
        tx.commit().map_err(SessionError::sqlite)?;
        Ok(last)
    }

    /// Releases the write lease eagerly (otherwise released on drop).
    ///
    /// # Errors
    /// Typed storage failure.
    pub fn release(&self) -> Result<(), SessionError> {
        if let Some(owner) = &self.lease_owner {
            let conn = self.store.lock_conn()?;
            lease::release(&conn, &self.meta.id, &owner.owner_id)?;
        }
        Ok(())
    }
}

impl Drop for SessionHandle<'_> {
    fn drop(&mut self) {
        if let Some(owner) = &self.lease_owner
            && let Ok(conn) = self.store.lock_conn()
        {
            let _ = lease::release(&conn, &self.meta.id, &owner.owner_id);
        }
    }
}

fn unlock(
    conn: &Connection,
    meta: &SessionMeta,
    passphrase: &str,
) -> Result<SessionKey, SessionError> {
    unlock_session_key(conn, &meta.id, passphrase)
}

/// One append inside the caller's transaction: event row, turn index,
/// counters row, FTS row. The caller owns `counters` for the whole
/// transaction: they must match the `sessions` row when the transaction
/// began, and this function advances them in memory — sequential
/// appends continue the sequence without re-reading the row (inside
/// one transaction SQLite is single-writer, so a re-read would return
/// exactly what the previous append wrote).
pub(crate) fn append_in_tx(
    conn: &Connection,
    session_id: &str,
    key: Option<&SessionKey>,
    event: &Event,
    now: u64,
    counters: &mut SessionCounters,
) -> Result<Sequence, SessionError> {
    let seq = counters.last_seq + 1;
    let recorded = RecordedEvent {
        seq: Sequence(seq as u64),
        event: event.clone(),
    };
    let json = serde_json::to_vec(&recorded).map_err(|error| SessionError::Serialize {
        detail: error.to_string(),
    })?;
    let payload = match key {
        Some(key) => key.seal(session_id.as_bytes(), &json)?,
        None => json,
    };
    let kind = kind_name(event);
    conn.prepare_cached(
        "INSERT INTO events (session_id, seq, kind, payload) VALUES (?1, ?2, ?3, ?4)",
    )
    .map_err(SessionError::sqlite)?
    .execute(rusqlite::params![session_id, seq, kind, payload])
    .map_err(SessionError::sqlite)?;
    let turns = index::update_on_append(conn, session_id, kind, seq as u64, counters.turn_count)?;
    if !counters.encrypted
        && let Some(text) = extract_text(event)
    {
        conn.prepare_cached("INSERT INTO event_fts (session_id, seq, text) VALUES (?1, ?2, ?3)")
            .map_err(SessionError::sqlite)?
            .execute(rusqlite::params![session_id, seq, text])
            .map_err(SessionError::sqlite)?;
    }
    conn.prepare_cached(
        "UPDATE sessions SET last_seq = ?2, event_count = event_count + 1,
         payload_bytes = payload_bytes + ?3, turn_count = ?4, updated_at = ?5 WHERE id = ?1",
    )
    .map_err(SessionError::sqlite)?
    .execute(rusqlite::params![
        session_id,
        seq,
        payload.len() as i64,
        turns,
        now as i64
    ])
    .map_err(SessionError::sqlite)?;
    counters.last_seq = seq;
    counters.turn_count = turns;
    Ok(Sequence(seq as u64))
}

fn kind_name(event: &Event) -> &'static str {
    match event {
        Event::User { .. } => "user",
        Event::Assistant { .. } => "assistant",
        Event::ToolCall { .. } => "tool_call",
        Event::ToolResult { .. } => "tool_result",
        Event::System { .. } => "system",
        Event::Condensation { .. } => "condensation",
        Event::Snapshot { .. } => "snapshot",
    }
}
