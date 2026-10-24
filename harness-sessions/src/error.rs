//! Typed session-store errors: every failure mode is a variant, never a
//! panic, never a stringly-typed retry.

use crate::lease::LeaseInfo;

/// Everything that can go wrong in the sessions subsystem.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SessionError {
    /// No session with this id exists in the store.
    #[error("session not found: {id}")]
    NotFound {
        /// The missing session id.
        id: String,
    },
    /// A session with this id already exists.
    #[error("session already exists: {id}")]
    AlreadyExists {
        /// The colliding session id.
        id: String,
    },
    /// The session id cannot be used as a directory name or store key.
    #[error("invalid session id {id:?}: {reason}")]
    InvalidId {
        /// The rejected id.
        id: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// The SQLite file is unreadable, not a database, or corrupt.
    #[error("session store corrupt: {detail}")]
    Corrupt {
        /// Underlying failure description.
        detail: String,
    },
    /// The store file carries a schema version newer than this build.
    #[error(
        "store schema {version} is newer than this build ({supported}); upgrade the harness first"
    )]
    Schema {
        /// Version found on disk.
        version: u32,
        /// Highest version this build understands.
        supported: u32,
    },
    /// A SQLite operation failed (non-corrupt operational failure).
    #[error("sqlite: {detail}")]
    Sqlite {
        /// Underlying failure description.
        detail: String,
    },
    /// A filesystem operation failed.
    #[error("io: {detail}")]
    Io {
        /// Underlying failure description.
        detail: String,
    },
    /// The turn index disagrees with the event table (deleted or
    /// inconsistent rows). [`SessionHandle::repair_index`][0] can rebuild
    /// it when only the derived index is damaged.
    ///
    /// [0]: crate::store::SessionHandle::repair_index
    #[error("turn index corrupt for session {id}: {detail}")]
    CorruptIndex {
        /// The affected session.
        id: String,
        /// What is inconsistent.
        detail: String,
    },
    /// The event table itself has sequence gaps or duplicates; the index
    /// cannot be rebuilt from a damaged log.
    #[error("event log damaged for session {id}: {detail}")]
    EventLogDamaged {
        /// The affected session.
        id: String,
        /// What is damaged.
        detail: String,
    },
    /// An encrypted session was opened without its passphrase.
    #[error("session {id} is encrypted; supply the passphrase")]
    MissingPassphrase {
        /// The locked session.
        id: String,
    },
    /// Decryption failed: wrong passphrase, tampered ciphertext, or a
    /// ciphertext moved between sessions (AAD binding). No partial
    /// plaintext is ever returned.
    #[error("decrypt failed for session {id}: {detail}")]
    Decrypt {
        /// The locked session.
        id: String,
        /// Why decryption failed.
        detail: String,
    },
    /// Encryption failed while writing.
    #[error("encrypt failed: {detail}")]
    Encrypt {
        /// Why encryption failed.
        detail: String,
    },
    /// The handle is a read-only view; appends require an OpenMode with
    /// a lease.
    #[error("session {id} is open read-only; appends need ReadWrite mode")]
    NotWritable {
        /// The read-only session.
        id: String,
    },
    /// A live lease held by another writer blocks write access. The
    /// session remains readable; never force-unlock — the other writer
    /// may still be alive.
    #[error("session {id} is locked by live writer {lease}")]
    LockedByLiveWriter {
        /// The locked session.
        id: String,
        /// The live lease.
        lease: LeaseInfo,
    },
    /// The requested fork point is past the end of the source log.
    #[error("fork point {at} is beyond the source log length {last}")]
    ForkPointBeyondLog {
        /// The requested fork sequence.
        at: u64,
        /// The source's last sequence.
        last: u64,
    },
    /// The requested truncation head is past the end of the log.
    #[error("truncate head {at} is beyond the log length {last} for session {id}")]
    TruncateBeyondLog {
        /// The affected session.
        id: String,
        /// The requested head sequence.
        at: u64,
        /// The log's actual last sequence.
        last: u64,
    },
    /// An append or resume targeted an archived session.
    #[error("session {id} is archived; restore it before writing")]
    Archived {
        /// The archived session.
        id: String,
    },
    /// A full-text query could not be parsed by FTS5.
    #[error("invalid search query {query:?}: {detail}")]
    BadQuery {
        /// The rejected query.
        query: String,
        /// Parser detail.
        detail: String,
    },
    /// The requested turn number does not exist in the session.
    #[error("session {id} has no turn {turn} (it has {turns})")]
    NoSuchTurn {
        /// The session.
        id: String,
        /// The requested turn.
        turn: u32,
        /// The number of turns that exist.
        turns: u32,
    },
    /// An import or export failed structurally (unreadable source,
    /// unwritable target).
    #[error("transfer failed: {detail}")]
    Transfer {
        /// What failed.
        detail: String,
    },
    /// An event failed to serialize into the store's line format.
    #[error("event serialization failed: {detail}")]
    Serialize {
        /// Why serialization failed.
        detail: String,
    },
}

impl SessionError {
    pub(crate) fn sqlite(error: rusqlite::Error) -> Self {
        Self::Sqlite {
            detail: error.to_string(),
        }
    }

    pub(crate) fn io(error: std::io::Error) -> Self {
        Self::Io {
            detail: error.to_string(),
        }
    }
}
