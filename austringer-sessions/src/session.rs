//! Lazy session handles: open reads metadata and index bounds only;
//! appends are transactional and leased.

use rusqlite::{Connection, OptionalExtension};

use austringer_core::event::{Event, RecordedEvent, Sequence};

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
        let seq = append_in_tx(
            &tx,
            &self.meta.id,
            self.key.as_ref(),
            &event,
            self.store.now(),
        )?;
        tx.commit().map_err(SessionError::sqlite)?;
        Ok(seq)
    }

    /// Appends many events in one transaction; returns the last sequence.
    ///
    /// # Errors
    /// Same typed conditions as [`Self::append`].
    pub fn append_batch(&self, events: &[Event]) -> Result<Sequence, SessionError> {
        self.assert_writable()?;
        let mut guard = self.store.lock_conn()?;
        let tx = guard.transaction().map_err(SessionError::sqlite)?;
        let mut last = Sequence(0);
        for event in events {
            last = append_in_tx(
                &tx,
                &self.meta.id,
                self.key.as_ref(),
                event,
                self.store.now(),
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
/// counters, FTS row. Reads the live counters row first, so sequential
/// appends through any handle continue the sequence correctly.
pub(crate) fn append_in_tx(
    conn: &Connection,
    session_id: &str,
    key: Option<&SessionKey>,
    event: &Event,
    now: u64,
) -> Result<Sequence, SessionError> {
    let (last_seq, turn_count, encrypted): (i64, i64, bool) = conn
        .query_row(
            "SELECT last_seq, turn_count, encrypted FROM sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0)),
        )
        .optional()
        .map_err(SessionError::sqlite)?
        .ok_or_else(|| SessionError::NotFound {
            id: session_id.to_owned(),
        })?;
    let seq = last_seq + 1;
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
    conn.execute(
        "INSERT INTO events (session_id, seq, kind, payload) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![session_id, seq, kind, payload],
    )
    .map_err(SessionError::sqlite)?;
    let turns = index::update_on_append(
        conn,
        session_id,
        kind,
        seq as u64,
        u32::try_from(turn_count).unwrap_or(u32::MAX),
    )?;
    if !encrypted && let Some(text) = extract_text(event) {
        conn.execute(
            "INSERT INTO event_fts (session_id, seq, text) VALUES (?1, ?2, ?3)",
            rusqlite::params![session_id, seq, text],
        )
        .map_err(SessionError::sqlite)?;
    }
    conn.execute(
        "UPDATE sessions SET last_seq = ?2, event_count = event_count + 1,
         payload_bytes = payload_bytes + ?3, turn_count = ?4, updated_at = ?5 WHERE id = ?1",
        rusqlite::params![session_id, seq, payload.len() as i64, turns, now as i64],
    )
    .map_err(SessionError::sqlite)?;
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
