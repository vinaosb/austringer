// Shared by multiple test binaries, none of which use every helper.
#![allow(dead_code)]

//! Fixtures for austringer-snapshots integration tests: deterministic git
//! repositories with USER commits (no austringer trailer), tree digests that
//! exclude austringer state, failure-injection probes, and a recording
//! rewind-log seam.

use austringer_snapshots::{RestoreProbe, SessionId, TruncationCursor, Turn};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- files

pub fn write_file(root: &Path, rel: &Path, bytes: &[u8]) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("mkdir {parent:?}: {e}"));
    }
    std::fs::write(&full, bytes).unwrap_or_else(|e| panic!("write {full:?}: {e}"));
}

pub fn delete_file(root: &Path, rel: &Path) {
    std::fs::remove_file(root.join(rel)).unwrap_or_else(|e| panic!("rm {rel:?}: {e}"));
}

fn walk_visible(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(root).unwrap_or_else(|e| panic!("read_dir {root:?}: {e}"));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("dir entry {root:?}: {e}"));
        let path = entry.path();
        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
            continue; // symlinks are skipped everywhere (T21 policy)
        }
        if path.is_dir() {
            if entry.file_name() == ".git" || entry.file_name() == ".austringer" {
                continue; // VCS metadata and austringer state are never user state
            }
            walk_visible(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Order-independent blake3 digest of every visible file's relative path +
/// bytes. The misleading-success guard: trees are compared by this, not by
/// spot checks.
pub fn tree_digest(root: &Path) -> [u8; 32] {
    let mut files = Vec::new();
    walk_visible(root, &mut files);
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
    let mut hasher = blake3::Hasher::new();
    for (rel, bytes) in by_rel {
        hasher.update(rel.as_bytes());
        hasher.update(&[0]);
        hasher.update(&bytes);
        hasher.update(&[0]);
    }
    hasher.finalize().into()
}

pub fn digest_hex(digest: &[u8; 32]) -> String {
    blake3::Hash::from_bytes(*digest).to_hex().to_string()
}

// ---------------------------------------------------------------- git

fn open_repo(workdir: &Path) -> gix::Repository {
    gix::open(workdir).unwrap_or_else(|e| panic!("open {workdir:?}: {e}"))
}

fn signature() -> gix::actor::Signature {
    gix::actor::Signature {
        name: "user".into(),
        email: "user@example.com".into(),
        time: gix::date::Time::now_utc(),
    }
}

enum TreeNode {
    Blob(gix::ObjectId),
    Dir(BTreeMap<String, TreeNode>),
}

/// Write a commit onto `refs/heads/main` with `message` (NO austringer
/// trailer unless the message itself carries one), tree built from `files`
/// (full replacement), parent = current main. Returns the commit hex.
pub fn commit(workdir: &Path, message: &str, files: &BTreeMap<String, Vec<u8>>) -> String {
    let repo = open_repo(workdir);
    let mut root: BTreeMap<String, TreeNode> = BTreeMap::new();
    for (rel, bytes) in files {
        let blob = repo.write_blob(bytes.as_slice()).expect("blob").detach();
        let parts: Vec<String> = Path::new(rel)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let mut cursor = &mut root;
        for part in &parts[..parts.len() - 1] {
            cursor = match cursor
                .entry(part.clone())
                .or_insert_with(|| TreeNode::Dir(BTreeMap::new()))
            {
                TreeNode::Dir(map) => map,
                TreeNode::Blob(_) => panic!("path is both file and dir: {rel}"),
            };
        }
        cursor.insert(parts[parts.len() - 1].clone(), TreeNode::Blob(blob));
    }
    fn write_nodes(repo: &gix::Repository, nodes: &BTreeMap<String, TreeNode>) -> gix::ObjectId {
        let entries: Vec<gix::objs::tree::Entry> = nodes
            .iter()
            .map(|(name, node)| {
                let (mode, oid) = match node {
                    TreeNode::Blob(oid) => (gix::objs::tree::EntryKind::Blob.into(), *oid),
                    TreeNode::Dir(map) => (
                        gix::objs::tree::EntryKind::Tree.into(),
                        write_nodes(repo, map),
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
            .expect("tree")
            .detach()
    }
    let tree = write_nodes(&repo, &root);
    let head =
        std::fs::read_to_string(workdir.join(".git").join("refs").join("heads").join("main"))
            .ok()
            .and_then(|text| text.trim().parse().ok());
    let parents: Vec<gix::ObjectId> = head.into_iter().collect();
    let commit = gix::objs::Commit {
        message: message.into(),
        tree,
        parents: parents.into(),
        author: signature(),
        committer: signature(),
        encoding: None,
        extra_headers: Vec::new(),
    };
    let id = repo
        .write_object(&commit)
        .expect("commit")
        .to_hex()
        .to_string();
    std::fs::write(
        workdir.join(".git").join("refs").join("heads").join("main"),
        format!("{id}\n"),
    )
    .expect("move main");
    id
}

/// A fixture repo with one USER commit on `refs/heads/main`.
pub fn git_repo(files: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let workdir = dir.path().to_path_buf();
    gix::init(&workdir).expect("gix init");
    std::fs::write(workdir.join(".git").join("HEAD"), "ref: refs/heads/main\n").expect("set HEAD");
    let mut map = BTreeMap::new();
    for (rel, bytes) in files {
        write_file(&workdir, Path::new(rel), bytes);
        map.insert((*rel).to_owned(), bytes.to_vec());
    }
    if !map.is_empty() {
        commit(&workdir, "user: initial", &map);
    }
    (dir, workdir)
}

pub fn head_hex(workdir: &Path) -> String {
    std::fs::read_to_string(workdir.join(".git").join("refs").join("heads").join("main"))
        .expect("read main")
        .trim()
        .to_owned()
}

// ---------------------------------------------------------------- seams

/// Fails the Nth file operation (0-based) to abort a restore mid-way.
pub struct FailingProbe {
    pub fail_at: Option<usize>,
    seen: Cell<usize>,
}

impl FailingProbe {
    pub fn never() -> Self {
        Self {
            fail_at: None,
            seen: Cell::new(0),
        }
    }
    pub fn at(index: usize) -> Self {
        Self {
            fail_at: Some(index),
            seen: Cell::new(0),
        }
    }
}

impl RestoreProbe for FailingProbe {
    fn before_file_op(&mut self, path: &Path) -> std::io::Result<()> {
        let n = self.seen.get();
        self.seen.set(n + 1);
        if self.fail_at == Some(n) {
            Err(std::io::Error::other(format!(
                "injected failure at {path:?}"
            )))
        } else {
            Ok(())
        }
    }
}

/// Records every truncation cursor the log seam receives.
#[derive(Default)]
pub struct RecordingLog {
    pub cursors: std::sync::Mutex<Vec<TruncationCursor>>,
}

impl austringer_snapshots::RewindLog for RecordingLog {
    fn truncate(
        &mut self,
        cursor: &TruncationCursor,
    ) -> Result<(), austringer_snapshots::SnapshotError> {
        self.cursors.lock().expect("log lock").push(cursor.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------- ids

pub fn sess(name: &str) -> SessionId {
    SessionId::new(name).expect("valid session id")
}

pub fn turn(n: u32) -> Turn {
    Turn::new(n).expect("valid turn")
}
