//! Typed error surface for shadow transactions (plan todo 21).
//!
//! Every failure mode the shadow layer can produce is named: no stringly
//! errors, no panics. Backings that are structurally unavailable are
//! [`VfsError::BackingUnavailable`]; user-tree races surface as
//! [`VfsError::BaseDrift`] so callers can re-read and re-apply instead of
//! guessing.

use std::path::PathBuf;

/// blake3 content hash of a file's bytes.
pub type FileHash = [u8; 32];

#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("{op} failed: {source}")]
    Io {
        op: &'static str,
        source: std::io::Error,
    },
    #[error("git operation {op} failed: {source}")]
    Git {
        op: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("shadow path {path:?} was rejected: {reason}")]
    InvalidShadowPath {
        path: PathBuf,
        reason: PathRejection,
    },
    #[error("edit batch is empty")]
    EmptyEdits,
    #[error("duplicate edit path {path:?}")]
    DuplicateEditPath { path: PathBuf },
    #[error("content hash mismatch at {path:?}: expected {expected}, found {actual}")]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("user tree drifted from the shadow base at {path:?} ({kind})")]
    BaseDrift { path: PathBuf, kind: DriftKind },
    #[error("commit failed (fully rolled back: {restored}): {source}")]
    CommitFailed {
        source: std::io::Error,
        restored: bool,
    },
    #[error("no edits applied; nothing to commit")]
    NothingApplied,
    #[error("shadow already retired")]
    AlreadyRetired,
    #[error("backing {requested:?} is unavailable for this target")]
    BackingUnavailable { requested: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathRejection {
    #[error("absolute, prefixed, or UNC paths are not shadow-relative")]
    Absolute,
    #[error("parent-directory components would escape the shadow root")]
    ParentTraversal,
    #[error("not a canonical relative path (empty, '.', or trailing cur-dir)")]
    NonCanonical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DriftKind {
    #[error("file content changed since the shadow base was taken")]
    Modified,
    #[error("file present in the shadow base is now missing")]
    Deleted,
    #[error("path the transaction wanted to create already exists")]
    Appeared,
    #[error("a file occupies a parent directory the transaction needs")]
    Conflict,
}
