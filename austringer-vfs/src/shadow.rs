//! The shadow handle: apply-inside-shadow, disclosure accessors, and
//! retire-to-reclaimed teardown (plan todo 21).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::edit::{ShadowEdit, resolve_batch};
use crate::error::VfsError;
use crate::manifest::{Manifest, hash_of};
use crate::paths;
use crate::store::{Backing, ShadowStore};
use crate::worktree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyStats {
    pub applied: usize,
    pub created: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitStats {
    pub written: usize,
    pub created: usize,
}

pub struct ShadowHandle<'store> {
    pub(crate) store: &'store ShadowStore,
    pub(crate) target: PathBuf,
    pub(crate) shadow_dir: PathBuf,
    pub(crate) id: String,
    pub(crate) backing: Backing,
    pub(crate) manifest: Manifest,
    pub(crate) applied: Vec<ShadowEdit>,
    pub(crate) retired: bool,
}

impl ShadowHandle<'_> {
    pub(crate) fn new<'a>(
        store: &'a ShadowStore,
        target: &Path,
        shadow_dir: PathBuf,
        id: String,
        backing: Backing,
        manifest: Manifest,
    ) -> ShadowHandle<'a> {
        ShadowHandle {
            store,
            target: target.to_path_buf(),
            shadow_dir,
            id,
            backing,
            manifest,
            applied: Vec::new(),
            retired: false,
        }
    }

    #[must_use]
    pub fn backing(&self) -> &Backing {
        &self.backing
    }

    #[must_use]
    pub fn shadow_path(&self) -> &Path {
        &self.shadow_dir
    }

    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Apply edits INSIDE the shadow. The user tree is never touched by
    /// this call; only [`Self::commit`] materializes into the user tree.
    pub fn apply(&mut self, edits: &[ShadowEdit]) -> Result<ApplyStats, VfsError> {
        if self.retired {
            return Err(VfsError::AlreadyRetired);
        }
        let resolved = resolve_batch(&self.applied, edits)?;
        for edit in &resolved {
            let shadow_file = self.shadow_dir.join(&edit.path);
            match edit.expected_hash {
                Some(expected) => {
                    let bytes = paths::read_file(&shadow_file).map_err(|source| {
                        VfsError::HashMismatch {
                            path: edit.path.clone(),
                            expected: crate::manifest::hex_of(&expected),
                            actual: format!("<unreadable: {source}>"),
                        }
                    })?;
                    let actual = hash_of(&bytes);
                    if actual != expected {
                        return Err(VfsError::HashMismatch {
                            path: edit.path.clone(),
                            expected: crate::manifest::hex_of(&expected),
                            actual: crate::manifest::hex_of(&actual),
                        });
                    }
                }
                None => {
                    if shadow_file.symlink_metadata().is_ok() {
                        return Err(VfsError::HashMismatch {
                            path: edit.path.clone(),
                            expected: "<absent>".to_owned(),
                            actual: "<file exists>".to_owned(),
                        });
                    }
                }
            }
        }
        let mut applied = 0;
        let mut created = 0;
        for edit in &resolved {
            let shadow_file = self.shadow_dir.join(&edit.path);
            if let Some(parent) = shadow_file.parent() {
                paths::create_dir_all(parent).map_err(|source| VfsError::Io {
                    op: "apply mkdir",
                    source,
                })?;
            }
            paths::write_file(&shadow_file, &edit.new_content).map_err(|source| VfsError::Io {
                op: "apply write",
                source,
            })?;
            if edit.expected_hash.is_some() {
                applied += 1;
            } else {
                created += 1;
            }
        }
        self.applied.extend(resolved);
        Ok(ApplyStats { applied, created })
    }

    /// Explicit rollback: retire the shadow now (equivalent to dropping
    /// the handle). The user tree is unchanged by construction.
    pub fn discard(&mut self) {
        self.retire();
    }

    /// Move the shadow into the store's reclaimed area with grace markers
    /// and, for gix backings, pin the base commit under
    /// `refs/austringer/reclaimed/<id>/<commit>`. Best-effort: teardown
    /// failures are logged, never fatal.
    pub(crate) fn retire(&mut self) {
        if self.retired {
            return;
        }
        self.retired = true;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = paths::write_file(
            &self.shadow_dir.join(".austringer-backing"),
            self.backing.name().as_bytes(),
        );
        let _ = paths::write_file(
            &self.shadow_dir.join(".austringer-retired-at"),
            now.to_string().as_bytes(),
        );
        if let Backing::GixWorktree { commit, repo } = &self.backing {
            let _ = paths::write_file(
                &self.shadow_dir.join(".austringer-commit"),
                commit.as_bytes(),
            );
            let _ = paths::write_file(
                &self.shadow_dir.join(".austringer-repo"),
                repo.as_os_str().as_encoded_bytes(),
            );
        }
        let reclaimed = self.store.root.join("reclaimed").join(&self.id);
        if rename_with_retry(&self.shadow_dir, &reclaimed) {
            if let Backing::GixWorktree { commit, repo } = &self.backing
                && let Err(error) = worktree::write_reclaimed_ref(repo, &self.id, commit)
            {
                tracing::warn!(shadow_id = %self.id, %error, "reclaimed ref write failed");
            }
        } else {
            tracing::warn!(shadow_id = %self.id, "retire rename failed; shadow stays in the active area for GC");
        }
    }
}

impl Drop for ShadowHandle<'_> {
    fn drop(&mut self) {
        self.retire();
    }
}

fn rename_with_retry(from: &Path, to: &Path) -> bool {
    for attempt in 0..3 {
        if paths::rename(from, to).is_ok() {
            return true;
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    false
}
