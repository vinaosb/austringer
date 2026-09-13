//! Gix committed-tree backing (plan todo 21): the NTFS / no-CoW fallback.
//!
//! gix 0.87 has no linked-worktree *creation* API, so this backing
//! materializes the HEAD subtree that `git worktree add` would check out:
//! the discovered repository's committed tree below the target's
//! repository-relative prefix, written through the always-on object
//! database. The shadow layer self-checks the result against the user-tree
//! manifest and only keeps this backing when the two match byte-for-byte
//! (clean worktree, no smudge-filter divergence); anything else demotes to
//! a full copy. Retired gix shadows pin their base commit under
//! `refs/harness/reclaimed/<shadow-id>/<commit>` for one GC cycle.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

use crate::error::{FileHash, VfsError};
use crate::manifest::{Manifest, hash_of};
use crate::paths;

pub(crate) struct GitBase {
    pub git_dir: PathBuf,
    pub commit_hex: String,
    pub subtree: gix::ObjectId,
}

pub(crate) fn discover_base(target: &Path) -> Option<GitBase> {
    let repo = gix::discover(target).ok()?;
    let workdir = repo.workdir()?.to_path_buf();
    let rel = target.strip_prefix(&workdir).ok()?;
    let commit = repo.head_commit().ok()?;
    let tree = repo.head_tree().ok()?;
    let subtree = subtree_id(&repo, tree.id().detach(), rel)?;
    Some(GitBase {
        git_dir: repo.git_dir().to_path_buf(),
        commit_hex: commit.id().to_hex().to_string(),
        subtree,
    })
}

fn subtree_id(repo: &gix::Repository, tree_id: gix::ObjectId, rel: &Path) -> Option<gix::ObjectId> {
    let mut current = tree_id;
    for component in rel.components() {
        let name = component.as_os_str().to_string_lossy();
        let tree = repo.find_tree(current).ok()?;
        let entry = tree
            .iter()
            .flatten()
            .find(|entry| entry.mode().is_tree() && entry.filename().to_str_lossy() == name)?;
        current = entry.oid().to_owned();
    }
    Some(current)
}

/// Write the committed subtree into `dst`, returning the manifest of files
/// actually materialized (for the fidelity self-check).
pub(crate) fn materialize(base: &GitBase, dst: &Path) -> Result<Manifest, VfsError> {
    let repo =
        gix::open(&base.git_dir).map_err(|source| git_error("open for materialize", source))?;
    let mut files: BTreeMap<PathBuf, FileHash> = BTreeMap::new();
    write_subtree(&repo, base.subtree, dst, Path::new(""), &mut files)?;
    Ok(Manifest { files })
}

fn git_error(op: &'static str, source: impl std::error::Error + Send + Sync + 'static) -> VfsError {
    VfsError::Git {
        op,
        source: Box::new(source),
    }
}

fn write_subtree(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    dst: &Path,
    prefix: &Path,
    files: &mut BTreeMap<PathBuf, FileHash>,
) -> Result<(), VfsError> {
    if !prefix.as_os_str().is_empty() {
        paths::create_dir_all(&dst.join(prefix)).map_err(|source| VfsError::Io {
            op: "materialize mkdir",
            source,
        })?;
    }
    let tree = repo
        .find_tree(tree_id)
        .map_err(|source| git_error("read tree", source))?;
    for entry in tree.iter() {
        let entry = entry.map_err(|source| git_error("tree entry", source))?;
        let name = entry.filename().to_str_lossy().into_owned();
        if entry.mode().is_tree() {
            write_subtree(
                repo,
                entry.oid().to_owned(),
                dst,
                &prefix.join(&name),
                files,
            )?;
            continue;
        }
        if entry.mode().is_link() {
            continue;
        }
        let blob = repo
            .find_blob(entry.oid().to_owned())
            .map_err(|source| git_error("read blob", source))?;
        let rel = prefix.join(&name);
        let target = dst.join(&rel);
        paths::write_file(&target, &blob.data).map_err(|source| VfsError::Io {
            op: "materialize write",
            source,
        })?;
        set_executable(&target, entry.mode().is_executable());
        files.insert(rel, hash_of(&blob.data));
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    if !executable {
        return;
    }
    if let Ok(meta) = std::fs::metadata(paths::extended(path)) {
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(paths::extended(path), perms);
    }
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) {}

pub(crate) fn reclaimed_ref_name(shadow_id: &str, commit_hex: &str) -> String {
    format!("refs/harness/reclaimed/{shadow_id}/{commit_hex}")
}

pub(crate) fn write_reclaimed_ref(
    git_dir: &Path,
    shadow_id: &str,
    commit_hex: &str,
) -> Result<(), VfsError> {
    let repo = gix::open(git_dir).map_err(|source| git_error("open for reclaimed ref", source))?;
    let target = gix::ObjectId::from_hex(commit_hex.as_bytes())
        .map_err(std::io::Error::other)
        .map_err(|source| VfsError::Io {
            op: "commit hex",
            source,
        })?;
    repo.reference(
        reclaimed_ref_name(shadow_id, commit_hex),
        target,
        gix::refs::transaction::PreviousValue::Any,
        "harness-vfs: retire shadow",
    )
    .map_err(|source| git_error("write reclaimed ref", source))?;
    Ok(())
}

pub(crate) fn delete_reclaimed_ref(
    git_dir: &Path,
    shadow_id: &str,
    commit_hex: &str,
) -> Result<(), VfsError> {
    let repo =
        gix::open(git_dir).map_err(|source| git_error("open for reclaimed ref delete", source))?;
    let name = reclaimed_ref_name(shadow_id, commit_hex);
    match repo.find_reference(name.as_str()) {
        Ok(reference) => {
            reference
                .delete()
                .map_err(|source| git_error("delete reclaimed ref", source))?;
            Ok(())
        }
        Err(_) => Ok(()),
    }
}
