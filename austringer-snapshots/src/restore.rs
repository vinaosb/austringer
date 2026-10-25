//! `/rewind`: plan building and the restore transaction.
//!
//! A [`RewindPlan`] is a pure value — the file-restore set, the
//! externally-modified skips, the event-truncation cursor, and the task
//! state for `FilesAndTask` mode. [`SnapshotStore::execute_rewind`] turns
//! it into a commit-or-rollback transaction: the user's dirty worktree is
//! stashed FIRST (git: a commit under `refs/austringer/restore-transactions/
//! <uuid>`; no-git: a CAS manifest), every file operation is backed up in
//! memory, and any failure rolls the whole batch back (`RestoreFailed`
//! carries whether the rollback completed). The truncation cursor is
//! emitted to the [`RewindLog`] seam only after a committed restore — the
//! session log layer (T4) consumes it; sessions/loop wire the concrete
//! handle later.

use std::collections::BTreeMap;
use std::path::Path;

use crate::ids::{SessionId, TruncationCursor, TxId, new_tx_id};
use crate::manifest::{ExternallyModifiedPolicy, RestoreMode, SnapshotMeta, TaskState};
use crate::{SnapshotError, Turn, util};

/// Where a restored file's bytes come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentSource {
    Blob { oid_hex: String },
    Cas { hash_hex: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWrite {
    pub rel: String,
    pub source: ContentSource,
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    ExternallyModified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    pub path: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone)]
pub struct RewindPlan {
    pub session: SessionId,
    pub turn: Turn,
    pub mode: RestoreMode,
    pub writes: Vec<FileWrite>,
    pub deletes: Vec<String>,
    pub skipped: Vec<SkippedFile>,
    pub event_cursor: TruncationCursor,
    pub task: Option<TaskState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashRef {
    /// Git ref (`refs/austringer/restore-transactions/<uuid>`) or the
    /// fallback's `.austringer/restore-transactions/<uuid>` directory.
    pub ref_name: String,
    pub tx: TxId,
}

#[derive(Debug, Clone)]
pub struct RestoreReport {
    pub restored: usize,
    pub deleted: usize,
    pub skipped: Vec<SkippedFile>,
    pub stash: Option<StashRef>,
    pub task: Option<TaskState>,
    pub rolled_back: bool,
}

/// The session-log seam: truncate history to the cursor. Mirrors the T4
/// `Sequence` semantics without depending on austringer-core.
pub trait RewindLog {
    fn truncate(&mut self, cursor: &TruncationCursor) -> Result<(), SnapshotError>;
}

/// Failure injection seam for restore-transaction tests (cancel/resume
/// channel): called before every file write and delete.
pub trait RestoreProbe {
    fn before_file_op(&mut self, path: &Path) -> std::io::Result<()>;
}

/// No-op probe.
impl RestoreProbe for () {
    fn before_file_op(&mut self, _path: &Path) -> std::io::Result<()> {
        Ok(())
    }
}

/// Grouped inputs for [`build_plan`] (the 3-parameter discipline).
pub(crate) struct PlanInputs<'a> {
    pub session: &'a SessionId,
    pub turn: Turn,
    pub mode: RestoreMode,
    pub policy: ExternallyModifiedPolicy,
    pub meta: &'a SnapshotMeta,
    /// Slashed relative path -> (content source, executable).
    pub snapshot_files: &'a BTreeMap<String, (ContentSource, bool)>,
    /// Current visible walk: path -> blake3 hex.
    pub current: &'a BTreeMap<String, String>,
    /// The austringer's latest observation (the Skip policy decides
    /// externality against it).
    pub baseline: Option<&'a SnapshotMeta>,
    pub through: Option<u64>,
}

pub(crate) fn build_plan(inputs: PlanInputs<'_>) -> RewindPlan {
    let PlanInputs {
        session,
        turn,
        mode,
        policy,
        meta,
        snapshot_files,
        current,
        baseline,
        through,
    } = inputs;
    let mut skipped = Vec::new();
    let mut writes = Vec::new();
    for (rel, (source, exec)) in snapshot_files {
        let current_hash = current.get(rel).map(String::as_str);
        let snap_hash = meta.files.get(rel).map(String::as_str);
        let differs = current_hash.is_none_or(|now| Some(now) != snap_hash);
        if !differs {
            continue;
        }
        if policy == ExternallyModifiedPolicy::Skip
            && baseline
                .map(|b| b.files.get(rel).map(String::as_str))
                .is_some_and(|base| current_hash != base)
        {
            skipped.push(SkippedFile {
                path: rel.clone(),
                reason: SkipReason::ExternallyModified,
            });
            continue;
        }
        writes.push(FileWrite {
            rel: rel.clone(),
            source: source.clone(),
            executable: *exec,
        });
    }
    let deletes = current
        .keys()
        .filter(|rel| !snapshot_files.contains_key(*rel))
        .cloned()
        .collect();
    RewindPlan {
        session: session.clone(),
        turn,
        mode,
        writes,
        deletes,
        skipped,
        event_cursor: TruncationCursor {
            session: session.clone(),
            turn,
            through: through.map(crate::EventCursor),
        },
        task: (mode == RestoreMode::FilesAndTask)
            .then(|| meta.task.clone())
            .flatten(),
    }
}

enum Backup {
    Write {
        path: std::path::PathBuf,
        prior: Option<Vec<u8>>,
    },
    Delete {
        path: std::path::PathBuf,
        bytes: Vec<u8>,
    },
}

pub(crate) struct RestoreTx {
    workdir: std::path::PathBuf,
    backups: Vec<Backup>,
    pub restored: usize,
    pub deleted: usize,
}

impl RestoreTx {
    pub(crate) fn new(workdir: &Path) -> Self {
        Self {
            workdir: workdir.to_path_buf(),
            backups: Vec::new(),
            restored: 0,
            deleted: 0,
        }
    }

    pub(crate) fn write(
        &mut self,
        rel: &str,
        bytes: &[u8],
        executable: bool,
        probe: &mut dyn RestoreProbe,
    ) -> Result<(), SnapshotError> {
        let path = self.workdir.join(rel);
        probe
            .before_file_op(&path)
            .map_err(|source| SnapshotError::RestoreFailed {
                source,
                restored: false,
            })?;
        let prior = util::read_file(&path).ok();
        if let Some(parent) = path.parent() {
            util::create_dir_all(parent)?;
        }
        util::write_file(&path, bytes)?;
        set_executable(&path, executable);
        self.backups.push(Backup::Write { path, prior });
        self.restored += 1;
        Ok(())
    }

    pub(crate) fn delete(
        &mut self,
        rel: &str,
        probe: &mut dyn RestoreProbe,
    ) -> Result<(), SnapshotError> {
        let path = self.workdir.join(rel);
        probe
            .before_file_op(&path)
            .map_err(|source| SnapshotError::RestoreFailed {
                source,
                restored: false,
            })?;
        let bytes = util::read_file(&path)?;
        util::remove_file(&path)?;
        self.backups.push(Backup::Delete { path, bytes });
        self.deleted += 1;
        Ok(())
    }

    /// Roll back every applied operation in reverse order; `true` when the
    /// rollback completed cleanly.
    pub(crate) fn rollback(mut self) -> (usize, usize, bool) {
        let mut clean = true;
        while let Some(backup) = self.backups.pop() {
            let outcome = match &backup {
                Backup::Write { path, prior } => match prior {
                    Some(bytes) => util::write_file(path, bytes),
                    None => {
                        if path.symlink_metadata().is_ok() {
                            util::remove_file(path)
                        } else {
                            Ok(())
                        }
                    }
                },
                Backup::Delete { path, bytes } => util::write_file(path, bytes),
            };
            if let Err(error) = outcome {
                clean = false;
                let at = match &backup {
                    Backup::Write { path, .. } | Backup::Delete { path, .. } => {
                        path.display().to_string()
                    }
                };
                tracing::warn!(%error, at, "restore rollback write failed");
            }
        }
        (self.restored, self.deleted, clean)
    }
}

fn set_executable(path: &Path, executable: bool) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if executable && let Ok(meta) = std::fs::metadata(util::extended(path)) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(util::extended(path), perms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, executable);
    }
}

/// New restore-transaction id.
pub(crate) fn new_transaction() -> Result<TxId, SnapshotError> {
    new_tx_id()
}
