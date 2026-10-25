//! Typed error surface for snapshots and rewind. No stringly failures, no
//! panics; every refusal the plan names is a variant (nested repo, user
//! commits after snapshot, torn/corrupt artifacts, busy coordinator).

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("io {op} failed: {source}")]
    Io {
        op: &'static str,
        source: std::io::Error,
    },
    #[error("git {op} failed: {source}")]
    Git {
        op: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("os entropy unavailable: {source}")]
    Entropy { source: std::io::Error },
    #[error("no git repository at {workdir:?}; the no-git fallback covers non-repo projects")]
    NotARepository { workdir: PathBuf },
    #[error("snapshot {session}/{turn} does not exist")]
    MissingSnapshot { session: String, turn: u32 },
    #[error("refusing restore: user commits exist after the snapshot: {commits:?}")]
    UserCommitsAfterSnapshot { commits: Vec<String> },
    #[error(
        "refusing snapshot: nested repository at {path:?} is not a registered submodule (never touched)"
    )]
    NestedRepoRefused { path: PathBuf },
    #[error("restore failed (fully rolled back: {restored}): {source}")]
    RestoreFailed {
        source: std::io::Error,
        restored: bool,
    },
    #[error("checkpoint manifest at {path:?} is torn (truncated write)")]
    TornCheckpoint { path: PathBuf },
    #[error("checkpoint manifest at {path:?} is corrupt (wrong shape)")]
    CorruptCheckpoint { path: PathBuf },
    #[error("snapshot manifest at {path:?} is torn (truncated write)")]
    TornSnapshot { path: PathBuf },
    #[error("snapshot manifest at {path:?} is corrupt (wrong shape)")]
    CorruptSnapshot { path: PathBuf },
    #[error("scratch index at {path:?} is torn or corrupt: {reason}")]
    ScratchIndex { path: PathBuf, reason: &'static str },
    #[error("coordinator is busy in state {state:?}; restores serialize through the state machine")]
    Busy { state: &'static str },
    #[error("signing failed: {source}")]
    SigningFailed {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("serialization failed: {0}")]
    Serialize(String),
}

impl SnapshotError {
    pub(crate) fn git(
        op: &'static str,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Git {
            op,
            source: Box::new(source),
        }
    }

    pub(crate) fn git_boxed(
        op: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self::Git { op, source }
    }

    pub(crate) fn io(op: &'static str, source: std::io::Error) -> Self {
        Self::Io { op, source }
    }
}

/// blake3 content hash of a file's bytes, hex-encoded in manifests.
pub type FileHash = String;
