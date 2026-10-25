//! Gitignore-respecting directory walk shared by the index and grep paths.
//!
//! Uses the `ignore` crate (ripgrep's walker) with standard filters:
//! `.gitignore`/`.git/info/exclude`/global gitignore honored inside git
//! repositories, hidden entries skipped, symlinks not followed. One
//! additional rule: the index directory itself (wherever the caller placed
//! it, hidden or not) is pruned so an index inside the corpus root can
//! never feed itself.

use std::collections::BTreeMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

/// One candidate file discovered by the walker.
#[derive(Debug, Clone)]
pub(crate) struct FileStat {
    /// Absolute path as yielded by the walker.
    pub abs: PathBuf,
    /// Root-relative path with `/` separators (the index document key).
    pub rel: String,
    /// Modification time in nanoseconds since the Unix epoch.
    pub mtime_nanos: i64,
    /// File size in bytes.
    pub size: u64,
}

/// Files that were deliberately not candidates.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SkipCounters {
    /// Walk entries that could not be stat'd or were not regular files.
    pub unreadable: u32,
}

/// Walks `root` honoring gitignore rules, excluding any directory equal to
/// `exclude_dir` (and its subtree). Unreadable entries are skipped and
/// counted, never fatal.
pub(crate) fn walk_files(root: &Path, exclude_dir: Option<&Path>) -> (Vec<FileStat>, SkipCounters) {
    let mut files = Vec::new();
    let mut skips = SkipCounters::default();

    let mut builder = ignore::WalkBuilder::new(root);
    builder.standard_filters(true);
    if let Some(excluded) = exclude_dir
        && let Ok(canonical) = excluded.canonicalize()
    {
        let filter = move |entry: &ignore::DirEntry| entry.path() != canonical;
        builder.filter_entry(filter);
    }

    for entry in builder.build() {
        let Ok(entry) = entry else {
            skips.unreadable += 1;
            continue;
        };
        let Ok(metadata) = entry.metadata() else {
            skips.unreadable += 1;
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Some(stat) = file_stat(root, entry.path(), &metadata) else {
            skips.unreadable += 1;
            continue;
        };
        files.push(stat);
    }
    (files, skips)
}

fn file_stat(root: &Path, abs: &Path, metadata: &Metadata) -> Option<FileStat> {
    let rel_path = abs.strip_prefix(root).ok()?;
    let rel = rel_path.to_string_lossy().replace('\\', "/");
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().clamp(0, i64::MAX as u128) as i64)
        .unwrap_or_default();
    Some(FileStat {
        abs: abs.to_path_buf(),
        rel,
        mtime_nanos: mtime,
        size: metadata.len(),
    })
}

/// Collects the walker output into a rel-path map for diffing.
pub(crate) fn stat_map(files: Vec<FileStat>) -> BTreeMap<String, FileStat> {
    files
        .into_iter()
        .map(|stat| (stat.rel.clone(), stat))
        .collect()
}
