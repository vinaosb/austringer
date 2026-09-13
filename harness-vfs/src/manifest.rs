//! Content manifests: the shadow-base snapshot the transaction contract is
//! verified against, plus recursive disk-usage accounting for GC.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{FileHash, VfsError};
use crate::paths;

pub(crate) struct Manifest {
    pub files: BTreeMap<PathBuf, FileHash>,
}

pub(crate) fn hash_of(bytes: &[u8]) -> FileHash {
    blake3::hash(bytes).into()
}

pub(crate) fn hex_of(hash: &FileHash) -> String {
    blake3::Hash::from_bytes(*hash).to_hex().to_string()
}

/// Regular files under `root`, hashed. Symlinks (file or directory) and
/// `.git` directories are skipped: a shadow mirrors the working tree, and
/// VCS metadata is never shadowed, hashed into the base, or written by a
/// commit (the plan's never-touch-user's-.git rule).
pub(crate) fn walk_tree(root: &Path) -> Result<Manifest, VfsError> {
    // Normalize to the extended form first: read_dir children inherit it,
    // and rel paths are stripped against the same prefix.
    let root = paths::extended(root);
    let mut files = BTreeMap::new();
    walk_into(&root, &root, &mut files)?;
    Ok(Manifest { files })
}

fn walk_into(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<PathBuf, FileHash>,
) -> Result<(), VfsError> {
    let children = paths::read_dir_sorted(dir).map_err(|source| VfsError::Io {
        op: "manifest walk",
        source,
    })?;
    for child in children {
        let meta =
            std::fs::symlink_metadata(paths::extended(&child)).map_err(|source| VfsError::Io {
                op: "manifest stat",
                source,
            })?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if child.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            walk_into(root, &child, files)?;
        } else if meta.is_file() {
            let bytes = paths::read_file(&child).map_err(|source| VfsError::Io {
                op: "manifest read",
                source,
            })?;
            let rel = child.strip_prefix(root).map_err(|source| VfsError::Io {
                op: "manifest relpath",
                source: std::io::Error::other(source),
            })?;
            files.insert(rel.to_path_buf(), hash_of(&bytes));
        }
    }
    Ok(())
}

/// Total bytes of regular files under `root` (symlinks skipped).
pub(crate) fn dir_size(root: &Path) -> u64 {
    let mut total = 0;
    let Ok(children) = paths::read_dir_sorted(root) else {
        return 0;
    };
    for child in children {
        let Ok(meta) = std::fs::symlink_metadata(paths::extended(&child)) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            total += dir_size(&child);
        } else if meta.is_file() {
            total += meta.len();
        }
    }
    total
}
