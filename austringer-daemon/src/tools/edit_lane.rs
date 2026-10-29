//! The in-process `edit_lines` write lane (todo 16 + todo 21 join):
//! staged semantic application plus a drift-checked commit, replacing the
//! old read→`apply_edits_path` direct write.
//!
//! Receipts:
//! - **Staged application**: edits are validated by
//!   [`austringer_tools::edit::apply_edits`] (anchors, uniqueness, caps;
//!   gate deferred), staged into entity hunk groups
//!   ([`austringer_tools::hunks::stage_hunks`], tree-sitter entities via
//!   `austringer-ast`), and re-materialized by
//!   [`austringer_tools::staged_apply::apply_staged`] with ALL-YES
//!   decisions and the real [`SyntaxGate`] — the gate rules over the
//!   exact bytes that will reach the disk, so per-hunk review decisions
//!   (a later wire wave) can drop in without changing this seam.
//! - **Why not a whole-tree shadow per edit**: `ShadowStore`/
//!   `ShadowHandle` (todo 21) mirror and re-verify the ENTIRE target
//!   tree — blake3 of every file at shadow creation and again at
//!   `commit` — so a per-edit shadow over the edited file's parent costs
//!   O(tree) per call (a root-level file edit hashes the whole repo
//!   twice) and fails the commit when any sibling file changes between
//!   read and write: spurious cross-file coupling between concurrent
//!   sessions. The single-file transaction below carries the SAME VFS
//!   drift contract through [`ShadowEdit`] (the documented tools→vfs
//!   mapping type): expected blake3 of the pre-edit bytes, typed
//!   hash-mismatch on mid-air drift, backup + rollback on a failed
//!   write — `vfs::commit` semantics collapsed to one file.
//! - `exec::preflight` (formatters, cargo check / tsc) stays unwired:
//!   the default lane runs the cheap in-process AST gate only; the deep
//!   pipeline is a future config-gated mode and never a default-on cost.

use std::path::{Path, PathBuf};

use austringer_tools::ToolsError;
use austringer_tools::edit::{LineEdit, apply_edits};
use austringer_tools::gate::SyntaxGate;
use austringer_tools::hunks::{
    EntityHunkGroup, EntitySpan, Replacement, entities_from_defs, stage_hunks,
};
use austringer_tools::lines::LineTable;
use austringer_tools::staged_apply::{EntityDecision, apply_staged};
use austringer_tools::window::WindowView;
use austringer_vfs::{FileHash, ShadowEdit, VfsError};

/// Lane failures: tool-contract rejections pass through; the commit adds
/// the VFS vocabulary (drift, failed write) so the model sees the same
/// typed texts the shadow layer would produce.
#[derive(Debug, thiserror::Error)]
pub enum LaneError {
    /// Rejections from the editing tools (anchor, size, gate, ...).
    #[error(transparent)]
    Tools(#[from] ToolsError),
    /// Filesystem failure while reading or writing the target.
    #[error("{op} failed: {source}")]
    Io {
        /// Failing operation label.
        op: &'static str,
        /// The underlying error.
        source: std::io::Error,
    },
    /// Drift or commit failure in the VFS-contract write.
    #[error(transparent)]
    Vfs(#[from] VfsError),
}

/// A validated, staged, gate-passed edit batch awaiting its commit.
#[derive(Debug, Clone)]
pub struct StagedEdit {
    /// Path as passed to the tool (used verbatim for the write).
    pub path: PathBuf,
    /// Model-facing bounded fringe — the tool's return value, unchanged
    /// in shape from the pre-lane `apply_edits_path` contract.
    pub fringe: WindowView,
    /// blake3 of the pre-edit bytes, hex (Diff event `before_hash`).
    pub before_hash: String,
    /// blake3 of the post-edit bytes, hex (Diff event `after_hash`).
    pub after_hash: String,
    /// Unified-diff-shaped hunk strings, one per staged replacement,
    /// entity-labeled (`@@ -3 +3 @@ fn name`) so the frontends' hunk
    /// review gates render them like worker diffs.
    pub hunks: Vec<String>,
    /// The VFS drift-contract carrier: expected pre-edit blake3 plus the
    /// exact replacement bytes.
    edit: ShadowEdit,
}

/// The successful commit's view: what the tool returns and what the
/// `SessionEvent::Diff` carries.
#[derive(Debug, Clone)]
pub struct CommittedEdit {
    /// Path as passed to the tool.
    pub path: PathBuf,
    /// blake3 of the pre-edit bytes, hex.
    pub before_hash: String,
    /// blake3 of the post-edit bytes, hex.
    pub after_hash: String,
    /// Rendered hunks.
    pub hunks: Vec<String>,
    /// Model-facing bounded fringe (the tool result body).
    pub fringe: WindowView,
}

fn entities_for(path: &Path, old: &LineTable) -> Vec<EntitySpan> {
    let Some(language) = austringer_ast::LanguageId::from_path(path) else {
        return Vec::new();
    };
    match austringer_ast::Analyzer::new().analyze_bytes(language, old.as_bytes().to_vec()) {
        Ok(analysis) => entities_from_defs(old, &analysis.definitions),
        // Staging granularity degrades to one top-level group: an
        // unparseable entity map must never fail the edit itself.
        Err(_) => Vec::new(),
    }
}

/// Stage `edits` against the current content of `path`: validate in
/// memory, group by entity, re-materialize through `apply_staged` under
/// the syntax gate. The file is not touched; [`commit_staged`] writes.
///
/// # Errors
/// tool-contract rejections, non-UTF-8 content, read failures.
pub fn staged_edit(path: &Path, edits: &[LineEdit]) -> Result<StagedEdit, LaneError> {
    let bytes = std::fs::read(path).map_err(|source| LaneError::Io {
        op: "edit read",
        source,
    })?;
    let old = LineTable::from_bytes(bytes.clone()).map_err(LaneError::Tools)?;
    // Validation-only pass (gate=None): the syntax gate runs once, in
    // the staged application, over the bytes that will be written.
    let outcome = apply_edits(path, &old, edits, None).map_err(LaneError::Tools)?;
    let entities = entities_for(path, &old);
    let groups = stage_hunks(&old, &outcome.table, &entities);
    let decisions = vec![EntityDecision::Yes; groups.len()];
    let gate = SyntaxGate::for_path(path);
    let staged = apply_staged(path, &old, &outcome.table, &groups, &decisions, Some(&gate))
        .map_err(LaneError::Tools)?;
    let before = blake3::hash(&bytes);
    let after = blake3::hash(staged.table.as_bytes());
    Ok(StagedEdit {
        path: path.to_path_buf(),
        fringe: outcome.fringe,
        before_hash: before.to_hex().to_string(),
        after_hash: after.to_hex().to_string(),
        hunks: render_hunks(&old, &staged.table, &groups),
        edit: ShadowEdit::replace(
            path.to_path_buf(),
            FileHash::from(*before.as_bytes()),
            staged.table.as_bytes().to_vec(),
        ),
    })
}

/// Commit a staged edit under the VFS drift contract: verify the file
/// still hashes to what staging read, write the replacement bytes, and
/// restore the on-disk content if the write itself fails. The user tree
/// is never left holding a partial write.
///
/// # Errors
/// typed drift ([`VfsError::HashMismatch`]) when the file moved between
/// staging and commit; [`VfsError::CommitFailed`] (with the restore
/// outcome) when the write fails.
pub fn commit_staged(staged: StagedEdit) -> Result<CommittedEdit, LaneError> {
    let expected = staged.edit.expected_hash.unwrap_or_default();
    let current = std::fs::read(&staged.path).map_err(|source| LaneError::Io {
        op: "commit read",
        source,
    })?;
    if *blake3::hash(&current).as_bytes() != expected {
        return Err(VfsError::HashMismatch {
            path: staged.path.clone(),
            expected: hex(&expected),
            actual: hex_of(&current),
        }
        .into());
    }
    if let Err(source) = std::fs::write(&staged.path, &staged.edit.new_content) {
        // Backup-restore mirror of vfs commit.rs: a restore that fails
        // on bytes that already match the backup still counts restored.
        let restored = match std::fs::write(&staged.path, &current) {
            Ok(()) => true,
            Err(_) => std::fs::read(&staged.path).is_ok_and(|now| now == current),
        };
        return Err(VfsError::CommitFailed { source, restored }.into());
    }
    Ok(CommittedEdit {
        path: staged.path,
        before_hash: staged.before_hash,
        after_hash: staged.after_hash,
        hunks: staged.hunks,
        fringe: staged.fringe,
    })
}

/// Event-facing path: relative to the daemon cwd when possible (the
/// Diff contract documents relative, forward-slash paths), always
/// forward-slashed.
pub(crate) fn display_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let rel = path.strip_prefix(&cwd).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

fn hex(hash: &FileHash) -> String {
    blake3::Hash::from_bytes(*hash).to_hex().to_string()
}

fn hex_of(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// One `@@ -a[,n] +b[,n] @@ [entity]` range side; count omitted at 1
/// (the worker/test hunk shape `@@ -1 +1 @@`).
fn side(sign: char, start: usize, end: usize) -> String {
    if end < start {
        return format!("{sign}{start},0");
    }
    let count = end - start + 1;
    if count == 1 {
        format!("{sign}{start}")
    } else {
        format!("{sign}{start},{count}")
    }
}

fn render_replacement(
    old: &LineTable,
    new: &LineTable,
    replacement: &Replacement,
    entity: Option<&EntitySpan>,
) -> String {
    // A pure deletion has no new-side coordinates; the old position is
    // the closest cheap approximation (frontends parse the @@ shape,
    // not the numbers).
    let new_side = if replacement.new_end == 0 {
        format!("+{},0", replacement.old_start)
    } else {
        side('+', replacement.new_start, replacement.new_end)
    };
    let mut out = format!(
        "@@ {} {new_side} @@",
        side('-', replacement.old_start, replacement.old_end)
    );
    if let Some(entity) = entity {
        out.push_str(&format!(" {} {}", entity.kind, entity.name));
    }
    if replacement.old_end != 0 {
        for line in replacement.old_start..=replacement.old_end {
            out.push_str("\n-");
            out.push_str(old.content_opt(line).unwrap_or_default());
        }
    }
    if replacement.new_end >= replacement.new_start && replacement.new_end != 0 {
        for line in replacement.new_start..=replacement.new_end {
            out.push_str("\n+");
            out.push_str(new.content_opt(line).unwrap_or_default());
        }
    }
    out
}

fn render_hunks(old: &LineTable, new: &LineTable, groups: &[EntityHunkGroup]) -> Vec<String> {
    groups
        .iter()
        .flat_map(|group| {
            group
                .replacements
                .iter()
                .map(|replacement| render_replacement(old, new, replacement, group.entity.as_ref()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_omits_count_at_one() {
        assert_eq!(side('-', 3, 3), "-3");
        assert_eq!(side('+', 2, 5), "+2,4");
    }

    #[test]
    fn display_path_forward_slashes() {
        let path = Path::new("a\\b\\c.rs");
        assert_eq!(display_path(path), "a/b/c.rs");
    }
}
