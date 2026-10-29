//! Sessions store: SQLite source of truth with encryption at rest,
//! lazy index-based loading, fork/resume, fail-closed writer leases,
//! cross-session search, and the curated-memory subsystem.
//!
//! Plan todo 24. Design notes that reviewers should know:
//!
//! * **SQLite wrapper**: rusqlite everywhere (the task-8 permission
//!   store precedent); the plan's "sqlx" wording is superseded — see
//!   the consolidation decision in `.omo/evidence/task-24-*`.
//! * **Lazy by construction**: `open_session*` reads one metadata row
//!   plus an O(log n) index check; turns page in through primary-key
//!   range scans. A 100 MB session opens in microseconds of data
//!   access (the perf test proves payloads are never touched).
//! * **Encryption trade-off**: encrypted sessions are excluded from the
//!   full-text index — indexing their plaintext would defeat
//!   encryption-at-rest. `search_events` covers plaintext sessions;
//!   memory notes are always plaintext curated knowledge.
//! * **Vector half**: [`vector::VectorIndex`] with an FTS5-only default
//!   (sqlite-vec cannot be vendored on this machine — probes recorded
//!   in the task-24 evidence; a vec0 impl drops in later).
//! * **Credentials**: this crate structurally persists no API keys and
//!   no OAuth tokens — no type in the store accepts them; per-session
//!   encryption keys live only as argon2-wrapped envelopes, and OS
//!   keyring wiring is the caller's via [`keystore::KeyStore`].
//! * **Retention**: the only deletion path is an explicit
//!   [`retention::RetentionPolicy`] handed to
//!   [`SessionStore::prune`](store::SessionStore::prune); pinned
//!   sessions are exempt; nothing ever deletes implicitly.
//!
//! The session-state directory contract is
//! `{session-root}/{id}/checkpoints/*.json + plan.md + files/ +
//! events.jsonl` (see [`layout`]).

#![deny(missing_docs)]

mod counters;
mod export;
mod fork;
mod import;
mod index;
mod keystore;
mod lifecycle;
mod meta;
mod migrations;
mod paging;
mod truncate;

pub mod crypto;
pub mod dream;
pub mod error;
pub mod foreign;
pub mod layout;
pub mod lease;
pub mod memory;
pub mod retention;
pub mod retrieval;
pub mod session;
pub mod store;
pub mod vector;

pub use austringer_core::event::Sequence;
pub use crypto::{KdfParams, SessionKey, WrappedKey};
pub use dream::{DreamGates, DreamOutcome, NotYet as DreamNotYet};
pub use error::SessionError;
pub use foreign::{ForeignRead, ForeignReason, UnknownForeignEntry};
pub use fork::ForkOptions;
pub use import::{ImportOrigin, ImportReport};
pub use keystore::{InMemoryKeyStore, KeyStore};
pub use lease::{
    FakeLeaseProbe, LeaseInfo, LeaseOwner, LeaseProbe, StaleReason, SysinfoLeaseProbe,
    TakeoverReport,
};
pub use memory::{MemoryNote, NoteInput};
pub use meta::{EventSearchHit, SessionMeta};
pub use retention::{ArchivedRecord, PruneReport, RetentionPolicy};
pub use retrieval::{MemoryHit, RecallConfig, RecallQuery, RecallTrigger};
pub use session::{OpenMode, SessionHandle};
pub use store::{Clock, SessionStore};
pub use vector::{FtsOnlyVectors, InMemoryVectorIndex, VectorIndex};

pub use layout::validate_id as validate_session_id;
pub use paging::TurnPage;
