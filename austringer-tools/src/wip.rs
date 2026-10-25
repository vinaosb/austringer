//! WIP-diff context mode (plan todo 16, clause 8 — Mentat
//! git-diff-anchored context): given the current work-in-progress diff
//! plus untracked files, produce the typed **anchoring context** a
//! "continue my work" turn starts from.
//!
//! The daemon later computes the diffs (gix) and untracked inventory and
//! wires them here; this module owns the descriptor and the compact
//! render (per-file hunk summary, never file bodies).

use crate::hunks::Replacement;
use std::path::PathBuf;

/// One modified file's staged hunk list (old/new 1-based inclusive line
/// ranges; `old_end == 0` = pure insertion, `new_end == 0` = pure
/// deletion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WipFileDiff {
    /// File path.
    pub path: PathBuf,
    /// Whether this entry describes an untracked file (hunks then cover
    /// the whole file).
    pub untracked: bool,
    /// Changed regions.
    pub hunks: Vec<Replacement>,
}

/// The typed anchoring context for a "continue my work" turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WipContext {
    /// Modified files with their hunks.
    pub files: Vec<WipFileDiff>,
    /// Untracked files with their line counts.
    pub untracked: Vec<(PathBuf, usize)>,
    /// Compact rendered context block.
    pub render: String,
}

fn hunk_delta(hunks: &[Replacement]) -> (usize, usize) {
    let mut added = 0usize;
    let mut removed = 0usize;
    for hunk in hunks {
        if hunk.old_end >= hunk.old_start {
            removed += hunk.old_end - hunk.old_start + 1;
        }
        if hunk.new_end >= hunk.new_start && hunk.new_end != 0 {
            added += hunk.new_end - hunk.new_start + 1;
        }
    }
    (added, removed)
}

/// Build the WIP anchoring context from modified-file hunks and untracked
/// files (path + line count).
#[must_use]
pub fn wip_context(modified: &[WipFileDiff], untracked: &[(PathBuf, usize)]) -> WipContext {
    let mut render = if modified.is_empty() && untracked.is_empty() {
        "WIP context: no work in progress".to_owned()
    } else {
        format!(
            "WIP context: {} modified, {} untracked",
            modified.len(),
            untracked.len()
        )
    };
    for file in modified {
        let (added, removed) = hunk_delta(&file.hunks);
        render.push_str(&format!(
            "\n{} {}: {} hunk(s), +{added}/-{removed}",
            if file.untracked { "??" } else { "M " },
            file.path.display(),
            file.hunks.len()
        ));
        for hunk in &file.hunks {
            render.push_str(&format!(
                " [{}]",
                if hunk.old_end == 0 {
                    format!("after {}", hunk.old_start)
                } else {
                    format!("{}-{}", hunk.old_start, hunk.old_end)
                }
            ));
        }
    }
    for (path, lines) in untracked {
        render.push_str(&format!(
            "\n?? {} (untracked, {lines} lines)",
            path.display()
        ));
    }
    WipContext {
        files: modified.to_vec(),
        untracked: untracked.to_vec(),
        render,
    }
}
