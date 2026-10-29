//! Bounded, gitignore-aware workspace walker: the source-file feed for
//! the tag/repomap pipeline. Same walking posture as the TUI omnibar
//! (`austringer-tui/src/complete.rs`): hidden files skipped, gitignore
//! respected without requiring a git checkout, per-entry failures
//! skipped — only an unusable root is an error.

use crate::grammar::grammar_for;
use crate::language::LanguageId;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Default cap on returned files per walk.
pub const MAX_WALKED_FILES: usize = 2_000;

/// Default per-file size cap (the walker feeds the parse pipeline;
/// oversized sources are the analyzer's typed error, so they never
/// reach it from here).
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Typed walker failures. Everything else (one unreadable directory,
/// one vanished file mid-walk) skips that entry — the walk is bounded
/// by contract, not a mirror of the tree.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WalkError {
    /// The workspace root does not exist (or is unreadable).
    #[error("workspace root does not exist: {0}")]
    RootMissing(PathBuf),
    /// The workspace root exists but is not a directory.
    #[error("workspace root is not a directory: {0}")]
    RootNotDirectory(PathBuf),
}

/// Caps for [`walk_workspace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkLimits {
    /// Maximum files returned (applied AFTER sorting, so truncation is
    /// deterministic).
    pub max_files: usize,
    /// Files larger than this many bytes are skipped.
    pub max_file_bytes: u64,
}

impl Default for WalkLimits {
    fn default() -> Self {
        WalkLimits {
            max_files: MAX_WALKED_FILES,
            max_file_bytes: MAX_FILE_BYTES,
        }
    }
}

/// Walk `root` and return the source files a compiled-in grammar can
/// analyze: paths relative to `root`, forward-slashed, sorted, capped
/// at `limits.max_files`. Files whose detected language has no
/// compiled-in grammar are skipped (they would only degrade to
/// [`crate::analyze::AnalyzeOutcome::NoGrammar`]).
///
/// # Errors
/// [`WalkError::RootMissing`] / [`WalkError::RootNotDirectory`] for an
/// unusable root; never for per-entry problems.
pub fn walk_workspace(root: &Path, limits: &WalkLimits) -> Result<Vec<PathBuf>, WalkError> {
    let metadata =
        std::fs::metadata(root).map_err(|_| WalkError::RootMissing(root.to_path_buf()))?;
    if !metadata.is_dir() {
        return Err(WalkError::RootNotDirectory(root.to_path_buf()));
    }
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .require_git(false)
        .build();
    let mut files = Vec::new();
    for entry in walker.flatten() {
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(language) = LanguageId::from_path(path) else {
            continue;
        };
        if grammar_for(language).is_none() {
            continue;
        }
        let Ok(entry_metadata) = entry.metadata() else {
            continue;
        };
        if entry_metadata.len() > limits.max_file_bytes {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        files.push(PathBuf::from(relative.to_string_lossy().replace('\\', "/")));
    }
    files.sort();
    files.truncate(limits.max_files);
    Ok(files)
}
