//! Shadow store: backing kinds, selection policy, and shadow-slot
//! allocation (plan todo 21). Selection order is reflink, then gix
//! committed-tree (kept only when it mirrors the user tree byte-for-byte),
//! then a plain full copy; whichever wins is disclosed by name.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::VfsError;
use crate::manifest::{Manifest, walk_tree};
use crate::paths;
use crate::reflink::{CopyOutcome, copy_tree, reflink_tree};
use crate::shadow::ShadowHandle;
use crate::worktree;

/// Which backing a shadow's files physically came from. Disclosed by every
/// handle; callers must never assume CoW where the name says otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backing {
    /// Per-file reflink (FICLONE / APFS clonefile / ReFS block clone):
    /// copy-up is copy-on-write at the filesystem level.
    Reflink,
    /// The committed HEAD subtree materialized through gix's object
    /// database; only selected when it matches the user tree byte-for-byte.
    GixWorktree { commit: String, repo: PathBuf },
    /// Plain recursive copy (last resort on non-CoW, non-git trees).
    FullCopy,
    /// The target tree was empty; nothing to mirror.
    Empty,
}

impl Backing {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Backing::Reflink => "reflink",
            Backing::GixWorktree { .. } => "gix-worktree",
            Backing::FullCopy => "full-copy",
            Backing::Empty => "empty",
        }
    }
}

/// Backing selection for [`ShadowStore::create_shadow_choice`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackingChoice {
    /// reflink, then gix committed-tree (if it mirrors the user tree),
    /// then full copy.
    Auto,
    Reflink,
    GixWorktree,
    FullCopy,
}

pub struct ShadowStore {
    pub(crate) root: PathBuf,
    counter: AtomicU64,
}

impl ShadowStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, VfsError> {
        let root = root.into();
        for sub in ["shadows", "reclaimed"] {
            paths::create_dir_all(&root.join(sub)).map_err(|source| VfsError::Io {
                op: "store init",
                source,
            })?;
        }
        Ok(Self {
            root,
            counter: AtomicU64::new(0),
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn create_shadow(&self, target: &Path) -> Result<ShadowHandle<'_>, VfsError> {
        self.create_shadow_choice(target, BackingChoice::Auto)
    }

    pub fn create_shadow_choice(
        &self,
        target: &Path,
        choice: BackingChoice,
    ) -> Result<ShadowHandle<'_>, VfsError> {
        let manifest = walk_tree(target)?;
        let (id, shadow_dir) = self.next_shadow_dir()?;
        let backing = self.select_backing(target, &manifest, &shadow_dir, choice)?;
        Ok(ShadowHandle::new(
            self, target, shadow_dir, id, backing, manifest,
        ))
    }

    fn select_backing(
        &self,
        target: &Path,
        manifest: &Manifest,
        shadow_dir: &Path,
        choice: BackingChoice,
    ) -> Result<Backing, VfsError> {
        if manifest.files.is_empty() {
            return Ok(Backing::Empty);
        }
        match choice {
            BackingChoice::Auto => {
                match reflink_tree(target, shadow_dir, manifest)? {
                    CopyOutcome::Reflinked => return Ok(Backing::Reflink),
                    CopyOutcome::FellBack => reset_dir(shadow_dir)?,
                }
                if let Some(base) = worktree::discover_base(target) {
                    let written = worktree::materialize(&base, shadow_dir)?;
                    if written.files == manifest.files {
                        return Ok(Backing::GixWorktree {
                            commit: base.commit_hex,
                            repo: base.git_dir,
                        });
                    }
                    reset_dir(shadow_dir)?;
                }
                copy_tree(target, shadow_dir, manifest)?;
                Ok(Backing::FullCopy)
            }
            BackingChoice::Reflink => match reflink_tree(target, shadow_dir, manifest)? {
                CopyOutcome::Reflinked => Ok(Backing::Reflink),
                CopyOutcome::FellBack => Err(VfsError::BackingUnavailable {
                    requested: "reflink",
                }),
            },
            BackingChoice::GixWorktree => match worktree::discover_base(target) {
                Some(base) => {
                    let written = worktree::materialize(&base, shadow_dir)?;
                    if written.files == manifest.files {
                        Ok(Backing::GixWorktree {
                            commit: base.commit_hex,
                            repo: base.git_dir,
                        })
                    } else {
                        Err(VfsError::BackingUnavailable {
                            requested: "gix-worktree",
                        })
                    }
                }
                None => Err(VfsError::BackingUnavailable {
                    requested: "gix-worktree",
                }),
            },
            BackingChoice::FullCopy => {
                copy_tree(target, shadow_dir, manifest)?;
                Ok(Backing::FullCopy)
            }
        }
    }

    fn next_shadow_dir(&self) -> Result<(String, PathBuf), VfsError> {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id = format!("{:012}-{:09}-{n}", now.as_secs(), now.subsec_nanos());
        let dir = self.root.join("shadows").join(&id);
        paths::create_dir_all(&dir).map_err(|source| VfsError::Io {
            op: "shadow init",
            source,
        })?;
        Ok((id, dir))
    }
}

fn reset_dir(dir: &Path) -> Result<(), VfsError> {
    paths::remove_dir_all(dir).map_err(|source| VfsError::Io {
        op: "reset partial shadow",
        source,
    })?;
    paths::create_dir_all(dir).map_err(|source| VfsError::Io {
        op: "reset partial shadow",
        source,
    })
}
