//! The SQLite session store: open, migration glue, lifecycle, search,
//! pinning, and the handle factory.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension};

use crate::error::SessionError;
use crate::layout::Layout;
use crate::lease::{LeaseOwner, SharedProbe, SysinfoLeaseProbe};
use crate::meta::{
    EventSearchHit, SessionMeta, load_meta_conn, search_events_conn, search_events_in_conn,
};
use crate::retrieval::RecallConfig;
use crate::session::OpenMode;
use crate::vector::VectorIndex;

/// Logical clock (epoch seconds in production; counters in tests).
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The session store. One SQLite file under a session root; handles
/// borrow the store, so all access serializes over one connection.
pub struct SessionStore {
    conn: Mutex<Connection>,
    clock: Clock,
    probe: SharedProbe,
    layout: Layout,
    pub(crate) vectors: Mutex<Option<Arc<dyn VectorIndex>>>,
    pub(crate) recall_config: Mutex<RecallConfig>,
}

impl std::fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionStore")
            .field("root", &self.layout.root)
            .finish_non_exhaustive()
    }
}
pub(crate) fn is_corrupt(detail: &str) -> bool {
    let lowered = detail.to_ascii_lowercase();
    lowered.contains("not a database")
        || lowered.contains("malformed")
        || lowered.contains("corrupt")
}

fn map_open_error(error: rusqlite::Error) -> SessionError {
    let detail = error.to_string();
    if is_corrupt(&detail) {
        SessionError::Corrupt { detail }
    } else {
        SessionError::Sqlite { detail }
    }
}

pub(crate) fn constraint(error: rusqlite::Error, id: &str) -> SessionError {
    match error {
        rusqlite::Error::SqliteFailure(ffi, _)
            if ffi.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            SessionError::AlreadyExists { id: id.to_owned() }
        }
        other => SessionError::sqlite(other),
    }
}

impl SessionStore {
    /// Opens (creating if absent) the store at `root` with the wall
    /// clock and production lease probe.
    ///
    /// # Errors
    /// Typed [`SessionError::Corrupt`] when the file is unusable, and
    /// [`SessionError::Schema`] for future schema versions.
    pub fn open(root: &Path) -> Result<Self, SessionError> {
        Self::open_with(
            root,
            Arc::new(wall_clock),
            Arc::new(SysinfoLeaseProbe::new()),
        )
    }

    /// Opens with an injected clock (deterministic tests).
    ///
    /// # Errors
    /// Same typed conditions as [`Self::open`].
    pub fn open_with_clock(root: &Path, clock: Clock) -> Result<Self, SessionError> {
        Self::open_with(root, clock, Arc::new(SysinfoLeaseProbe::new()))
    }

    /// Opens with injected clock and lease probe (simulated reboots).
    ///
    /// # Errors
    /// Same typed conditions as [`Self::open`].
    pub fn open_with(root: &Path, clock: Clock, probe: SharedProbe) -> Result<Self, SessionError> {
        std::fs::create_dir_all(root).map_err(SessionError::io)?;
        let layout = Layout::new(root);
        let mut conn = Connection::open(layout.db_path()).map_err(map_open_error)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
        )
        .map_err(map_open_error)?;
        crate::migrations::migrate(&mut conn).map_err(|error| match error {
            SessionError::Sqlite { detail } if is_corrupt(&detail) => {
                SessionError::Corrupt { detail }
            }
            other => other,
        })?;
        Ok(Self {
            conn: Mutex::new(conn),
            clock,
            probe,
            layout,
            vectors: Mutex::new(None),
            recall_config: Mutex::new(RecallConfig::default()),
        })
    }

    pub(crate) fn lock_conn(&self) -> Result<MutexGuard<'_, Connection>, SessionError> {
        self.conn.lock().map_err(|_| SessionError::Sqlite {
            detail: "store mutex poisoned".to_owned(),
        })
    }

    pub(crate) fn layout(&self) -> &Layout {
        &self.layout
    }

    pub(crate) fn probe(&self) -> &SharedProbe {
        &self.probe
    }

    pub(crate) fn now(&self) -> u64 {
        (self.clock)()
    }

    pub(crate) fn load_meta(&self, id: &str) -> Result<SessionMeta, SessionError> {
        let conn = self.lock_conn()?;
        load_meta_conn(&conn, id)
    }

    /// Opens a session read-only (metadata + index check only; turns
    /// page in on demand). Encrypted sessions need
    /// [`Self::open_session_unlocked`].
    ///
    /// # Errors
    /// Typed: not found, corrupt index, missing passphrase.
    pub fn open_session(
        &self,
        id: &str,
    ) -> Result<crate::session::SessionHandle<'_>, SessionError> {
        self.open_session_mode(id, OpenMode::ReadOnly)
    }

    /// Opens a session for writing under `owner`'s lease. A live
    /// foreign lease is a typed [`SessionError::LockedByLiveWriter`];
    /// replacing a stale one surfaces a takeover report on the handle.
    ///
    /// # Errors
    /// Typed: not found, corrupt index, live foreign writer, archived,
    /// missing passphrase.
    pub fn open_session_rw(
        &self,
        id: &str,
        owner: &LeaseOwner,
    ) -> Result<crate::session::SessionHandle<'_>, SessionError> {
        self.open_session_mode(id, OpenMode::ReadWrite(owner.clone()))
    }

    /// Opens with an explicit [`OpenMode`].
    ///
    /// # Errors
    /// Union of [`Self::open_session`] and [`Self::open_session_rw`].
    pub fn open_session_mode(
        &self,
        id: &str,
        mode: OpenMode,
    ) -> Result<crate::session::SessionHandle<'_>, SessionError> {
        crate::session::SessionHandle::open(self, id, &mode, None)
    }

    /// Unlocks an encrypted session with its passphrase (either open
    /// mode). A wrong passphrase is a clean typed decrypt error.
    ///
    /// # Errors
    /// Typed: not found, wrong passphrase, corrupt index.
    pub fn open_session_unlocked(
        &self,
        id: &str,
        passphrase: &str,
        mode: OpenMode,
    ) -> Result<crate::session::SessionHandle<'_>, SessionError> {
        crate::session::SessionHandle::open(self, id, &mode, Some(passphrase))
    }

    /// Lists all sessions, newest first.
    ///
    /// # Errors
    /// Typed storage failure.
    pub fn list(&self) -> Result<Vec<SessionMeta>, SessionError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM sessions ORDER BY updated_at DESC, id",
                crate::meta::META_COLUMNS
            ))
            .map_err(SessionError::sqlite)?;
        let rows = stmt
            .query_map([], crate::meta::row_to_meta)
            .map_err(SessionError::sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::sqlite)?;
        Ok(rows)
    }

    /// Cross-session BM25 search over indexed (plaintext) event text.
    /// Encrypted sessions are excluded by design — an index of their
    /// plaintext would defeat encryption-at-rest.
    ///
    /// # Errors
    /// Typed [`SessionError::BadQuery`] for unparseable FTS5 syntax.
    pub fn search_events(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<EventSearchHit>, SessionError> {
        let conn = self.lock_conn()?;
        search_events_conn(&conn, query, limit)
    }

    /// Session-scoped BM25 search: only the listed session ids are
    /// searched, over the same event_fts index and ranking idiom as
    /// [`Self::search_events`]. An empty `session_ids` scope matches
    /// nothing (fail-closed).
    ///
    /// # Errors
    /// Typed [`SessionError::BadQuery`] for unparseable FTS5 syntax.
    pub fn search_events_in_sessions(
        &self,
        query: &str,
        session_ids: &[&str],
        limit: u32,
    ) -> Result<Vec<EventSearchHit>, SessionError> {
        let conn = self.lock_conn()?;
        search_events_in_conn(&conn, query, session_ids, limit)
    }

    /// The last event sequence of `turn` (`None` when the session has
    /// no such turn) — the rewind head when no checkpoint recorded one.
    ///
    /// # Errors
    /// Typed storage failure.
    pub fn turn_end_seq(&self, id: &str, turn: u32) -> Result<Option<u64>, SessionError> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT end_seq FROM turn_index WHERE session_id = ?1 AND turn = ?2",
            rusqlite::params![id, turn],
            |row| Ok(u64::try_from(row.get::<_, i64>(0)?).unwrap_or(u64::MAX)),
        )
        .optional()
        .map_err(SessionError::sqlite)
    }

    /// Sets or clears the pinned exemption.
    ///
    /// # Errors
    /// Typed: not found, storage.
    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<SessionMeta, SessionError> {
        {
            let conn = self.lock_conn()?;
            let changed = conn
                .execute(
                    "UPDATE sessions SET pinned = ?2 WHERE id = ?1",
                    rusqlite::params![id, pinned as i64],
                )
                .map_err(SessionError::sqlite)?;
            if changed == 0 {
                return Err(SessionError::NotFound { id: id.to_owned() });
            }
        }
        self.load_meta(id)
    }
}

pub(crate) fn state_bump(conn: &Connection, key: &str, by: i64) -> Result<(), SessionError> {
    conn.execute(
        "INSERT INTO store_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = value + ?2",
        rusqlite::params![key, by],
    )
    .map(|_| ())
    .map_err(SessionError::sqlite)
}

pub(crate) fn state_get(conn: &Connection, key: &str) -> Result<i64, SessionError> {
    conn.query_row(
        "SELECT value FROM store_state WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
    .map_err(SessionError::sqlite)
    .map(|value| value.unwrap_or(0))
}

fn wall_clock() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |span| span.as_secs())
}
