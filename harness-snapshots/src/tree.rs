//! In-memory tree assembly and reading: the gix object-database side of
//! snapshots. Trees are built bottom-up from walked files as
//! `gix::objs::Tree` values and written straight to the object database —
//! the user's index and working tree are never touched.

use std::collections::BTreeMap;
use std::path::Path;

use crate::{SnapshotError, util};

use gix::bstr::ByteSlice as _;

pub(crate) enum TreeNode {
    Blob { oid: gix::ObjectId, exec: bool },
    Commit(gix::ObjectId),
    Dir(BTreeMap<String, TreeNode>),
}

pub(crate) fn tree_entries(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    prefix: &str,
    out: &mut BTreeMap<String, (String, u32)>,
) -> Result<(), SnapshotError> {
    let tree = repo
        .find_tree(tree_id)
        .map_err(|source| SnapshotError::git("read tree", source))?;
    for entry in tree.iter() {
        let entry = entry.map_err(|source| SnapshotError::git("tree entry", source))?;
        let name = entry.filename().to_str_lossy().into_owned();
        if entry.mode().is_tree() {
            tree_entries(
                repo,
                entry.oid().to_owned(),
                &format!("{prefix}{name}/"),
                out,
            )?;
        } else if !entry.mode().is_link() {
            let mode = mode_of(entry.mode());
            out.insert(
                format!("{prefix}{name}"),
                (entry.oid().to_hex().to_string(), mode),
            );
        }
    }
    Ok(())
}

pub(crate) fn insert_tree(root: &mut BTreeMap<String, TreeNode>, rel: &str, leaf: TreeNode) {
    let path = Path::new(rel);
    let parts: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let mut cursor = root;
    for part in &parts[..parts.len() - 1] {
        cursor = match cursor
            .entry(part.clone())
            .or_insert_with(|| TreeNode::Dir(BTreeMap::new()))
        {
            TreeNode::Dir(map) => map,
            TreeNode::Blob { .. } | TreeNode::Commit(_) => return,
        };
    }
    cursor.insert(parts[parts.len() - 1].clone(), leaf);
}

pub(crate) fn write_tree(
    repo: &gix::Repository,
    nodes: &BTreeMap<String, TreeNode>,
) -> Result<gix::ObjectId, SnapshotError> {
    let mut entries = Vec::new();
    for (name, node) in nodes {
        let (kind, oid) = match node {
            TreeNode::Blob { oid, exec } => (
                if *exec {
                    gix::objs::tree::EntryKind::BlobExecutable
                } else {
                    gix::objs::tree::EntryKind::Blob
                },
                *oid,
            ),
            TreeNode::Commit(oid) => (gix::objs::tree::EntryKind::Commit, *oid),
            TreeNode::Dir(map) => (gix::objs::tree::EntryKind::Tree, write_tree(repo, map)?),
        };
        entries.push(gix::objs::tree::Entry {
            mode: kind.into(),
            filename: name.as_str().into(),
            oid,
        });
    }
    repo.write_object(&gix::objs::Tree { entries })
        .map_err(|source| SnapshotError::git("write tree", source))
        .map(|id| id.detach())
}

pub(crate) fn parse_oid(hex: &str) -> gix::ObjectId {
    gix::ObjectId::from_hex(hex.as_bytes())
        .unwrap_or_else(|_| gix::ObjectId::null(gix::hash::Kind::Sha1))
}

/// Raw git mode bits of a tree entry (100644 / 100755 / 160000).
fn mode_of(mode: gix::objs::tree::EntryMode) -> u32 {
    if mode.is_executable() {
        0o100755
    } else if mode.is_commit() {
        0o160000
    } else {
        0o100644
    }
}

pub(crate) fn harness_signature() -> gix::actor::Signature {
    gix::actor::Signature {
        name: "harness-agent".into(),
        email: "agent@harness.invalid".into(),
        time: gix::date::Time::now_utc(),
    }
}

/// Stash the current visible worktree state as a commit under
/// `refs/harness/restore-transactions/<uuid>` — the safety net the user
/// can recover from if a restore discards wanted changes.
pub(crate) fn stash_current(
    repo: &gix::Repository,
    workdir: &Path,
    tx: &crate::TxId,
) -> Result<String, SnapshotError> {
    let walked = crate::walk::walk(workdir, &[])?;
    let mut root: BTreeMap<String, TreeNode> = BTreeMap::new();
    for (rel, path) in &walked.files {
        let bytes = util::read_file(path)?;
        let oid = repo
            .write_blob(bytes.as_slice())
            .map_err(|source| SnapshotError::git("stash blob", source))?
            .detach();
        insert_tree(
            &mut root,
            rel,
            TreeNode::Blob {
                oid,
                exec: walked.executables.contains(rel),
            },
        );
    }
    let tree_id = write_tree(repo, &root)?;
    let head = repo.head_commit().ok().map(|commit| commit.id);
    let commit = gix::objs::Commit {
        message: format!("harness restore stash {}", tx.0).into(),
        tree: tree_id,
        parents: head.into_iter().collect(),
        author: harness_signature(),
        committer: harness_signature(),
        encoding: None,
        extra_headers: Vec::new(),
    };
    let commit_id = repo
        .write_object(&commit)
        .map_err(|source| SnapshotError::git("stash commit", source))?
        .detach();
    let name = crate::ids::transaction_ref(tx);
    repo.reference(
        name.as_str(),
        commit_id,
        gix::refs::transaction::PreviousValue::Any,
        "harness-snapshots: restore transaction stash",
    )
    .map_err(|source| SnapshotError::git("stash ref", source))?;
    Ok(name)
}
