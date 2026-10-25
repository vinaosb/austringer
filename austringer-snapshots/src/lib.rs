//! Snapshot checkpoints and `/rewind` time travel (plan todo 23).
//!
//! One engine, two backends behind [`SnapshotStore`]:
//!
//! * **git** — tree/commit objects built IN MEMORY through gitoxide and
//!   written to the object database, referenced from
//!   `refs/austringer/snapshots/{session}/{turn}` (plus a `.json` manifest
//!   sidecar ref). The user's branches, index, and `.git` structure are
//!   never renamed or mutated — the only `.git` writes are refs under
//!   `refs/austringer/`.
//! * **no-git** — content-addressed file snapshots under
//!   `.austringer/snapshots/` (reflink/copy CoW where the filesystem
//!   allows); git-only features surface as disabled
//!   [`SnapshotCapabilities`].
//!
//! # Safety model
//!
//! * Restore is a TRANSACTION: the user's dirty worktree is stashed to a
//!   private ref (`refs/austringer/restore-transactions/<uuid>`) or CAS
//!   manifest FIRST; every file operation is backed up in memory; any
//!   failure rolls the whole batch back. A restore is REFUSED while
//!   non-agent commits sit on the user's branch after the snapshot
//!   ([`SnapshotError::UserCommitsAfterSnapshot`] — the Cline 4.1.17
//!   lesson: never knock user commits off their branch).
//! * Hygiene (Roo): excludes are honored read-only; austringer built-ins
//!   layer in memory. Nested non-submodule repos are refused, never
//!   touched. Submodules are FROZEN POINTERS (gitlink entries; dirty
//!   state preserved as a pointer+patch record; never recursed).
//!   `.gitattributes` filter-managed files (LFS) are recorded and stored
//!   raw — no filter ever runs, restores are byte-identical.
//! * Provenance (Amp): snapshot commits carry a `Austringer-Session-Id`
//!   trailer, `Co-authored-by` attribution, and a `Austringer-Agent-Commit`
//!   workflow-approval marker (`provenance::requires_human_approval` is
//!   the CI predicate). Detached signing lives behind
//!   [`provenance::SigningService`] — keys stay OUT of agent processes.
//! * `/rewind` restores files AND truncates event history: the typed
//!   [`TruncationCursor`] is emitted to the [`RewindLog`] seam after a
//!   committed restore; the session log (T4) and loop wire the concrete
//!   handle in later todos.
//!
//! # Performance
//!
//! The persistent scratch stat-cache (a real git index v2 file per
//! session under `.austringer/scratch/<session>/index`, the Cline pattern)
//! skips re-hashing unchanged untracked files; a 14-day reaper collects
//! stale scratch directories. Checkpoint writes are INCREMENTAL: only the
//! new transcript entries of a turn are persisted
//! ([`checkpoint::CheckpointWriter`]).

pub mod checkpoint;
pub mod coordinator;
pub mod error;
pub mod gc;
pub mod ids;
pub mod manifest;
pub mod provenance;
pub mod scratch;

mod execute;
mod gitrefs;
mod gitstore;
mod nogit;
mod restore;
mod store;
mod tree;
mod util;
mod walk;

pub use checkpoint::{
    CHECKPOINTS_DIR, CheckpointMeta, CheckpointOrigin, CheckpointRecord, CheckpointRecorder,
    CheckpointWriter, ResumePoint, SessionThreadRecorder, TranscriptEntry,
};
pub use coordinator::{CheckpointCoordinator, CoordinatorState, DirtyCheck};
pub use error::SnapshotError;
pub use gc::{GcOutcome, SnapshotGcPolicy};
pub use ids::{EventCursor, InvalidId, SessionId, TruncationCursor, Turn, TxId};
pub use manifest::{
    ExternallyModifiedPolicy, RestoreMode, SnapshotCapabilities, SnapshotMeta, SnapshotOptions,
    SnapshotOutcome, StatCacheStats, SubmoduleRecord, TaskState,
};
pub use restore::{
    ContentSource, FileWrite, RestoreProbe, RestoreReport, RewindLog, RewindPlan, SkipReason,
    SkippedFile, StashRef,
};
pub use store::SnapshotStore;
