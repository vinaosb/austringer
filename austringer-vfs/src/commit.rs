//! The commit transaction (plan todo 21): the ONLY public path that writes
//! into the user tree. Verify drift against the base manifest, back up
//! replace-targets in memory, copy the edits up, and roll everything back
//! if any single write fails.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::edit::ShadowEdit;
use crate::error::{DriftKind, VfsError};
use crate::manifest::walk_tree;
use crate::paths;
use crate::shadow::{CommitStats, ShadowHandle};

impl ShadowHandle<'_> {
    pub fn commit(&mut self) -> Result<CommitStats, VfsError> {
        if self.retired {
            return Err(VfsError::AlreadyRetired);
        }
        if self.applied.is_empty() {
            return Err(VfsError::NothingApplied);
        }
        self.verify_no_drift()?;

        let mut written = 0;
        let mut created = 0;
        let mut originals: Vec<(PathBuf, Vec<u8>)> = Vec::new();
        let mut created_files: Vec<PathBuf> = Vec::new();
        let mut created_dirs: Vec<PathBuf> = Vec::new();
        for edit in &self.applied {
            let user_file = self.target.join(&edit.path);
            if let Err(source) = self.stage_and_write(
                edit,
                &user_file,
                &mut originals,
                &mut created_files,
                &mut created_dirs,
            ) {
                let restored = self.rollback(&originals, &created_files, &created_dirs);
                return Err(VfsError::CommitFailed { source, restored });
            }
            if edit.expected_hash.is_some() {
                written += 1;
            } else {
                created += 1;
            }
        }
        self.retire();
        Ok(CommitStats { written, created })
    }

    fn stage_and_write(
        &self,
        edit: &ShadowEdit,
        user_file: &Path,
        originals: &mut Vec<(PathBuf, Vec<u8>)>,
        created_files: &mut Vec<PathBuf>,
        created_dirs: &mut Vec<PathBuf>,
    ) -> Result<(), std::io::Error> {
        match edit.expected_hash {
            Some(_) => {
                let bytes = paths::read_file(user_file)?;
                originals.push((edit.path.clone(), bytes));
            }
            None => {
                ensure_parents(user_file, created_dirs)?;
                created_files.push(edit.path.clone());
            }
        }
        paths::write_file(user_file, &edit.new_content)
    }

    /// Restore originals and remove created files. A restore that fails
    /// on a file whose current bytes already match the backup (the write
    /// never landed, e.g. it was rejected on a read-only file) still
    /// counts as restored.
    fn rollback(
        &self,
        originals: &[(PathBuf, Vec<u8>)],
        created_files: &[PathBuf],
        created_dirs: &[PathBuf],
    ) -> bool {
        let mut restored = true;
        for (rel, bytes) in originals {
            let user_file = self.target.join(rel);
            let matches_backup =
                || paths::read_file(&user_file).is_ok_and(|current| current == *bytes);
            if paths::write_file(&user_file, bytes).is_err() && !matches_backup() {
                restored = false;
            }
        }
        for rel in created_files {
            let user_file = self.target.join(rel);
            if std::fs::remove_file(paths::extended(&user_file)).is_err()
                && user_file.symlink_metadata().is_ok()
            {
                restored = false;
            }
        }
        for dir in created_dirs.iter().rev() {
            let _ = std::fs::remove_dir(paths::extended(&self.target.join(dir)));
        }
        restored
    }

    fn verify_no_drift(&self) -> Result<(), VfsError> {
        let current = walk_tree(&self.target)?;
        let by_path: BTreeMap<&Path, &ShadowEdit> =
            self.applied.iter().map(|e| (e.path.as_path(), e)).collect();
        for (rel, base_hash) in &self.manifest.files {
            match by_path.get(rel.as_path()) {
                Some(edit) => match current.files.get(rel) {
                    Some(actual) if edit.expected_hash == Some(*actual) => {}
                    Some(_) => {
                        return Err(drift(rel, DriftKind::Modified));
                    }
                    None => return Err(drift(rel, DriftKind::Deleted)),
                },
                None => {
                    if current.files.get(rel) != Some(base_hash) {
                        return Err(drift(
                            rel,
                            if current.files.contains_key(rel) {
                                DriftKind::Modified
                            } else {
                                DriftKind::Deleted
                            },
                        ));
                    }
                }
            }
        }
        for edit in &self.applied {
            if edit.expected_hash.is_some() {
                continue;
            }
            let user_file = self.target.join(&edit.path);
            if user_file.symlink_metadata().is_ok() || current.files.contains_key(&edit.path) {
                return Err(drift(&edit.path, DriftKind::Appeared));
            }
            for ancestor in edit.path.ancestors().skip(1) {
                if current.files.contains_key(ancestor) {
                    return Err(drift(&edit.path, DriftKind::Conflict));
                }
            }
        }
        Ok(())
    }
}

fn drift(rel: &Path, kind: DriftKind) -> VfsError {
    VfsError::BaseDrift {
        path: rel.to_path_buf(),
        kind,
    }
}

/// Create `user_file`'s parent directories, recording the directories that
/// did not exist before so rollback can remove them again.
fn ensure_parents(user_file: &Path, created_dirs: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let Some(parent) = user_file.parent() else {
        return Ok(());
    };
    let mut missing = Vec::new();
    let mut cursor = Some(parent.to_path_buf());
    while let Some(dir) = cursor {
        if dir.symlink_metadata().is_ok() {
            break;
        }
        missing.push(dir.clone());
        cursor = dir.parent().map(Path::to_path_buf);
    }
    paths::create_dir_all(parent)?;
    created_dirs.extend(missing);
    Ok(())
}
