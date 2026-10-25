//! Hygiene walk (Roo): the one directory traversal every snapshot, restore
//! plan, and dirty-check goes through.
//!
//! * User excludes are honored READ-ONLY: `.gitignore` files, `.ignore`,
//!   and `.git/info/exclude` as parsed by the `ignore` crate. Austringer
//!   built-ins (`.austringer/`) and caller extras are layered IN MEMORY as
//!   overrides — the user's `.gitignore` and `.git` are never written (the
//!   "shadow info/exclude" realized as an in-memory layer, strictly
//!   stronger than Roo's on-disk `.git/info/exclude` append).
//! * `.git` directories and symlinks are skipped everywhere (T21 policy);
//!   dotfiles are included (git semantics).
//! * `.gitattributes` awareness: paths matched by a `filter=` rule (LFS)
//!   are recorded as pointer-managed — bytes are always stored raw, no
//!   filter is ever executed.
//! * A directory containing `.git` is a submodule (registered in
//!   `.gitmodules`) — recorded as a FROZEN POINTER with a dirty record,
//!   never recursed — or a stray nested repo, which refuses the whole
//!   snapshot (`NestedRepoRefused`, never touched).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::{SnapshotError, util};

use gix::bstr::ByteSlice as _;

#[derive(Debug, Default)]
pub(crate) struct WalkOutput {
    /// Visible regular files: slashed relative path -> absolute path.
    pub files: BTreeMap<String, PathBuf>,
    /// Executable-bit paths (unix only; always empty on Windows).
    pub executables: Vec<String>,
    /// Registered submodules: frozen pointer + dirty record.
    pub gitlinks: Vec<Gitlink>,
    /// Paths matched by a `.gitattributes` `filter=` rule (LFS pointers).
    pub filter_managed: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Gitlink {
    pub rel: String,
    pub commit_hex: String,
    /// (submodule-relative path, blake3 hex of current bytes) for files
    /// whose content drifted from the submodule's HEAD — the preserved
    /// pointer+patch record.
    pub dirty: Vec<(String, String)>,
}

/// Austringer state dirs this crate creates; they must never enter a snapshot.
pub(crate) const BUILT_IN_EXCLUDES: &[&str] = &["!.austringer"];

pub(crate) fn walk(root: &Path, extra_excludes: &[String]) -> Result<WalkOutput, SnapshotError> {
    let mut walker = ignore::WalkBuilder::new(root);
    walker
        .standard_filters(true)
        .hidden(false)
        .parents(true)
        .require_git(false)
        .follow_links(false);
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for pattern in BUILT_IN_EXCLUDES
        .iter()
        .copied()
        .chain(extra_excludes.iter().map(String::as_str))
    {
        overrides.add(pattern).map_err(|source| {
            SnapshotError::io("exclude pattern", std::io::Error::other(source))
        })?;
    }
    walker.overrides(
        overrides
            .build()
            .map_err(|source| SnapshotError::io("overrides", std::io::Error::other(source)))?,
    );

    // `.git` holders are pruned whole: a submodule stays a frozen pointer,
    // a stray nested repo is classified (and refused) after the walk.
    let git_holders: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let prune_state = Arc::clone(&git_holders);
    let walk_root = root.to_path_buf();
    walker.filter_entry(move |entry| {
        let path = entry.path();
        if path == walk_root || !entry.file_type().is_some_and(|kind| kind.is_dir()) {
            return true;
        }
        if entry.file_name() == ".git" {
            return false;
        }
        if path.join(".git").symlink_metadata().is_ok() {
            if let Ok(mut held) = prune_state.lock() {
                held.push(path.to_path_buf());
            }
            return false;
        }
        true
    });

    let gitmodules = parse_gitmodule_paths(&root.join(".gitmodules"));
    let attributes = parse_filter_patterns(&root.join(".gitattributes"));
    let mut out = WalkOutput::default();
    for entry in walker.build() {
        let entry = entry.map_err(|source| {
            SnapshotError::io("walk", std::io::Error::other(source.to_string()))
        })?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path().to_path_buf();
        let rel = rel_slash(&path, root)?;
        if is_executable(&path) {
            out.executables.push(rel.clone());
        }
        out.files.insert(rel, path);
    }

    let holders = git_holders.lock().map_err(|_| {
        SnapshotError::io(
            "walk state",
            std::io::Error::other("git-holder list poisoned"),
        )
    })?;
    let mut gitlinks = Vec::new();
    for holder in holders.iter() {
        let rel = rel_slash(holder, root)?;
        if !gitmodules.contains(&rel) {
            return Err(SnapshotError::NestedRepoRefused {
                path: holder.to_path_buf(),
            });
        }
        gitlinks.push(gitlink_record(holder, &rel)?);
    }
    out.gitlinks = gitlinks;
    if attributes.patterns > 0 {
        out.filter_managed = out
            .files
            .keys()
            .filter(|rel| attributes.matches(rel))
            .cloned()
            .collect();
    }
    Ok(out)
}

fn rel_slash(path: &Path, root: &Path) -> Result<String, SnapshotError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|source| SnapshotError::io("relpath", std::io::Error::other(source)))?;
    Ok(util::slash(rel))
}

fn gitlink_record(dir: &Path, rel: &str) -> Result<Gitlink, SnapshotError> {
    let sub = gix::open(dir).map_err(|source| SnapshotError::git("open submodule", source))?;
    let commit_hex = sub
        .head_commit()
        .map_err(|source| SnapshotError::git("submodule HEAD", source))?
        .id
        .to_hex()
        .to_string();
    let dirty = submodule_dirty(&sub, dir)?;
    Ok(Gitlink {
        rel: rel.to_owned(),
        commit_hex,
        dirty,
    })
}

/// The submodule's frozen-pointer dirty record. Read-only against the
/// submodule: blob ids are COMPUTED (sha1 of the would-be object), never
/// written into the nested object database.
fn submodule_dirty(
    sub: &gix::Repository,
    dir: &Path,
) -> Result<Vec<(String, String)>, SnapshotError> {
    let Ok(tree) = sub.head_tree() else {
        return Ok(Vec::new());
    };
    let mut head_blobs = BTreeMap::new();
    let mut stack = vec![(tree.id().detach(), String::new())];
    while let Some((tree_id, prefix)) = stack.pop() {
        let tree = sub
            .find_tree(tree_id)
            .map_err(|source| SnapshotError::git("submodule tree", source))?;
        for entry in tree.iter().flatten() {
            let name = entry.filename().to_str_lossy().into_owned();
            if entry.mode().is_tree() {
                stack.push((entry.oid().to_owned(), format!("{prefix}{name}/")));
            } else if !entry.mode().is_link() {
                head_blobs.insert(format!("{prefix}{name}"), entry.oid().to_owned());
            }
        }
    }
    let mut dirty = Vec::new();
    for (rel, path) in walk(dir, &[])?.files {
        let Some(head_oid) = head_blobs.get(&rel) else {
            continue;
        };
        let bytes = util::read_file(&path)?;
        if computed_blob_id(&bytes) != *head_oid {
            dirty.push((rel, blake3::hash(&bytes).to_hex().to_string()));
        }
    }
    Ok(dirty)
}

/// `sha1("blob <len>\0" || content)` — the git blob object id, computed
/// without touching any object database.
fn computed_blob_id(bytes: &[u8]) -> gix::ObjectId {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    hasher
        .try_finalize()
        .unwrap_or_else(|_| gix::ObjectId::null(gix::hash::Kind::Sha1))
}

/// Registered submodule paths from `.gitmodules` (`[submodule "x"] path = y`).
pub(crate) fn parse_gitmodule_paths(file: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("path ="))
        .map(|value| value.trim().to_owned())
        .filter(|path| !path.is_empty())
        .collect()
}

struct FilterPatterns {
    matcher: ignore::gitignore::Gitignore,
    patterns: usize,
}

impl FilterPatterns {
    fn matches(&self, rel: &str) -> bool {
        self.patterns > 0 && self.matcher.matched(rel, false).is_ignore()
    }
}

/// `filter=lfs`-style rules from the root `.gitattributes`. Pattern
/// semantics reuse the gitignore matcher (the dialects coincide);
/// awareness only — no filter is ever executed, bytes stay raw.
fn parse_filter_patterns(file: &Path) -> FilterPatterns {
    let mut builder =
        ignore::gitignore::GitignoreBuilder::new(file.parent().unwrap_or(Path::new(".")));
    let mut count = 0;
    if let Ok(text) = std::fs::read_to_string(file) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line
                .split_whitespace()
                .any(|attr| attr.starts_with("filter="))
                && line
                    .split_whitespace()
                    .next()
                    .is_some_and(|pattern| builder.add_line(None, pattern).is_ok())
            {
                count += 1;
            }
        }
    }
    FilterPatterns {
        matcher: builder
            .build()
            .unwrap_or_else(|_| ignore::gitignore::Gitignore::empty()),
        patterns: count,
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(util::extended(path))
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}
