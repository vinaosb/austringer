// Shared by multiple test binaries, none of which use every helper.
#![allow(dead_code)]

//! Shared fixtures for harness-vfs integration tests: deterministic tree
//! builders, content digests for the user-tree-unchanged invariant, and a
//! gix-backed fixture repository factory.

use blake3::Hasher;
use harness_vfs::ShadowEdit;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Deterministic content for file index `i` (CRLF every 5th file so the
/// byte-fidelity paths are exercised everywhere).
pub fn fixture_content(i: usize) -> Vec<u8> {
    if i.is_multiple_of(5) {
        format!("line {i}\r\ncontent {i}\r\nmore {i}\r\n").into_bytes()
    } else {
        format!("line {i}\ncontent {i}\nmore {i}\n").into_bytes()
    }
}

/// Deterministic relative paths: `d<i%7>/f{i}.txt`.
pub fn fixture_rel(i: usize) -> String {
    format!("d{}/f{i}.txt", i % 7)
}

/// Write `n` deterministic files under `root`.
pub fn write_fixture_tree(root: &Path, n: usize) -> Vec<String> {
    let mut rels = Vec::new();
    for i in 0..n {
        let rel = fixture_rel(i);
        write_file(root, Path::new(&rel), &fixture_content(i));
        rels.push(rel);
    }
    rels
}

pub fn write_file(root: &Path, rel: &Path, bytes: &[u8]) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("mkdir {parent:?}: {e}"));
    }
    std::fs::write(&full, bytes).unwrap_or_else(|e| panic!("write {full:?}: {e}"));
}

fn walk_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(root).unwrap_or_else(|e| panic!("read_dir {root:?}: {e}"));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("dir entry {root:?}: {e}"));
        let path = entry.path();
        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
            continue; // content digest covers regular files only
        }
        if path.is_dir() {
            if entry.file_name() == ".git" {
                continue; // working tree only: VCS metadata is not shadowed
            }
            walk_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Order-independent digest of every regular file's relative path + bytes.
/// The misleading-success guard: user trees are compared by this before and
/// after every scenario.
pub fn tree_digest(root: &Path) -> [u8; 32] {
    let mut files = Vec::new();
    walk_files(root, &mut files);
    let mut by_rel: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or_else(|e| panic!("strip {path:?}: {e}"))
            .to_string_lossy()
            .replace('\\', "/");
        by_rel.insert(
            rel,
            std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}")),
        );
    }
    let mut hasher = Hasher::new();
    for (rel, bytes) in by_rel {
        hasher.update(rel.as_bytes());
        hasher.update(&[0]);
        hasher.update(&bytes);
        hasher.update(&[0]);
    }
    hasher.finalize().into()
}

pub fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    blake3::hash(bytes).into()
}

pub fn hex(hash: &[u8; 32]) -> String {
    blake3::Hash::from_bytes(*hash).to_hex().to_string()
}

/// Replace edits for the first `count` fixture files.
pub fn fixture_replace_edits(count: usize) -> Vec<ShadowEdit> {
    (0..count)
        .map(|i| {
            let rel = fixture_rel(i);
            let new = format!("edited {i}\nsecond {i}\n").into_bytes();
            ShadowEdit::replace(PathBuf::from(rel), hash_bytes(&fixture_content(i)), new)
        })
        .collect()
}

/// Extended-length (`\\?\`) form of an absolute path so tests can read
/// deep trees regardless of the OS long-path setting. Forward slashes
/// are normalized: verbatim paths reject them. Windows-only mechanism —
/// on unix the verbatim form is not a valid path.
pub fn extended(path: &Path) -> PathBuf {
    if !cfg!(windows) {
        return path.to_path_buf();
    }
    let text = path.to_string_lossy();
    if text.starts_with(r"\\?\") || !path.is_absolute() {
        return path.to_path_buf();
    }
    PathBuf::from(format!(r"\\?\{}", text.replace('/', "\\")))
}

/// A fixture git repository whose HEAD tree contains exactly `files`,
/// with a matching clean worktree at the returned path. Owns its tempdir.
pub struct GixFixture {
    _dir: tempfile::TempDir,
    pub workdir: PathBuf,
    pub commit_hex: String,
}

pub fn git_fixture(files: &[(&str, &[u8])]) -> GixFixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let workdir = dir.path().to_path_buf();
    let repo = gix::init(&workdir).expect("gix init");
    for (rel, bytes) in files {
        write_file(&workdir, Path::new(rel), bytes);
    }
    let tree_id = build_tree(&repo, files);
    let signature = gix::actor::Signature {
        name: "fixture".into(),
        email: "fixture@example.com".into(),
        time: gix::date::Time::now_utc(),
    };
    let commit = gix::objs::Commit {
        message: "fixture".into(),
        tree: tree_id,
        parents: Vec::new().into(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        extra_headers: Vec::new(),
    };
    let commit_id = repo.write_object(&commit).expect("write commit");
    std::fs::write(workdir.join(".git").join("HEAD"), "ref: refs/heads/main\n").expect("set HEAD");
    // Plain loose ref (repo.reference would demand a committer identity
    // for the reflog; fixture repos have none).
    let heads = workdir.join(".git").join("refs").join("heads");
    std::fs::create_dir_all(&heads).expect("refs dir");
    std::fs::write(heads.join("main"), format!("{}\n", commit_id.to_hex())).expect("set main");
    GixFixture {
        _dir: dir,
        workdir,
        commit_hex: commit_id.to_hex().to_string(),
    }
}

enum TreeNode {
    Blob(gix::ObjectId),
    Dir(std::collections::BTreeMap<String, TreeNode>),
}

/// Build the nested tree object hierarchy proper to git (entries are
/// single-level names; slashes only exist through nesting).
fn build_tree(repo: &gix::Repository, files: &[(&str, &[u8])]) -> gix::ObjectId {
    let mut root: std::collections::BTreeMap<String, TreeNode> = std::collections::BTreeMap::new();
    for (rel, bytes) in files {
        let blob = repo.write_blob(*bytes).expect("write blob").detach();
        let parts: Vec<String> = Path::new(rel)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let mut cursor = &mut root;
        for part in &parts[..parts.len() - 1] {
            let entry = cursor
                .entry(part.clone())
                .or_insert_with(|| TreeNode::Dir(std::collections::BTreeMap::new()));
            match entry {
                TreeNode::Dir(map) => cursor = map,
                TreeNode::Blob(_) => panic!("fixture path is both file and directory: {rel}"),
            }
        }
        cursor.insert(parts[parts.len() - 1].clone(), TreeNode::Blob(blob));
    }
    write_tree_node(repo, &root)
}

fn write_tree_node(
    repo: &gix::Repository,
    nodes: &std::collections::BTreeMap<String, TreeNode>,
) -> gix::ObjectId {
    let entries: Vec<gix::objs::tree::Entry> = nodes
        .iter()
        .map(|(name, node)| {
            let (mode, oid) = match node {
                TreeNode::Blob(oid) => (gix::objs::tree::EntryKind::Blob.into(), *oid),
                TreeNode::Dir(map) => (
                    gix::objs::tree::EntryKind::Tree.into(),
                    write_tree_node(repo, map),
                ),
            };
            gix::objs::tree::Entry {
                mode,
                filename: name.as_str().into(),
                oid,
            }
        })
        .collect();
    repo.write_object(&gix::objs::Tree { entries })
        .expect("write tree")
        .detach()
}
