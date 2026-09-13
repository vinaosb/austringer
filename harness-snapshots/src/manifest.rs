//! Cross-backend snapshot vocabulary: options, manifests, capability
//! flags, outcomes, and the restore-mode enums. The JSON shapes are
//! stability contracts (sorted keys; no OS paths — slashed relatives).

use crate::ids::SessionId;
use crate::provenance::DetachedSignature;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::Turn;

/// What a snapshot recorded beyond the raw tree.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TaskState {
    /// The task/plan cursor at snapshot time.
    pub cursor: u64,
    /// `plan.md` bytes (T24 session-state contract).
    pub plan: Option<Vec<u8>>,
}

/// Options are not `Debug`-derived: the optional signer handle is a trait
/// object carrying host-side key material.
#[derive(Clone, Default)]
pub struct SnapshotOptions {
    /// Additional in-memory exclude patterns (gitignore dialect, e.g.
    /// `build/`); harness built-ins are always applied.
    pub extra_excludes: Vec<String>,
    /// Task cursor + plan state recorded with the snapshot.
    pub task: Option<TaskState>,
    /// Optional detached signing service (keys stay outside agent
    /// processes).
    pub signer: Option<Arc<dyn crate::provenance::SigningService>>,
}

/// Git-only features surfaced to the caller so degraded (no-git) projects
/// are honest about what they got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCapabilities {
    /// Namespace refs (`refs/harness/snapshots/...`) are available.
    pub refs: bool,
    /// Provenance trailers on commits are available.
    pub provenance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatCacheStats {
    pub hashed: usize,
    pub reused: usize,
}

#[derive(Debug, Clone)]
pub struct SnapshotOutcome {
    pub session: SessionId,
    pub turn: Turn,
    /// Git ref (`refs/harness/snapshots/...`) when the backend is git.
    pub ref_name: Option<String>,
    /// CAS manifest path when the backend is the no-git fallback.
    pub manifest_path: Option<PathBuf>,
    pub stats: StatCacheStats,
}

/// The frozen-pointer record for one registered submodule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmoduleRecord {
    pub path: String,
    pub commit_hex: String,
    /// (path, blake3 hex) of drifted files — the preserved patch record.
    pub dirty: Vec<(String, String)>,
}

/// The snapshot sidecar: everything restore needs that is not a blob.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SnapshotMeta {
    pub session: String,
    pub turn: u32,
    /// HEAD commit at snapshot time (None when unborn); the anchor the
    /// user-commits-after-snapshot check walks back to.
    pub head_at_snapshot: Option<String>,
    /// Slashed relative path -> blake3 hex.
    pub files: BTreeMap<String, String>,
    pub executables: Vec<String>,
    pub submodules: Vec<SubmoduleRecord>,
    /// LFS/filter-managed paths (stored raw; never smudged).
    pub filter_managed: Vec<String>,
    /// Cheap dirty-check fingerprint at snapshot time.
    pub stat_fingerprint: u64,
    pub task: Option<TaskState>,
    pub signature: Option<DetachedSignature>,
    pub recorded_unix: u64,
}

/// The two restore modes: files-only, or files + task cursor/plan state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    FilesOnly,
    FilesAndTask,
}

/// Copilot skip mode: files changed outside the harness since the last
/// snapshot are left alone and reported (`Skip`), or restored anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternallyModifiedPolicy {
    Skip,
    Include,
}
