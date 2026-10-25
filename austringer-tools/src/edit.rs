//! `edit_lines` behavior (plan todo 16, clause 2): anchored line edits with
//! uniqueness validation, AST-validated application, and bounded results.
//!
//! Contract (implements the day-one `edit_lines` tool schema fixed by
//! `austringer-core::tools::schema` in todo 6 — dispatch wiring lands later):
//! - each edit anchors at a 1-based `line` with `old_text` that must match
//!   the file exactly there (logical-line comparison, so model-side LF
//!   matches a CRLF file);
//! - the `old_text` sequence must occur **exactly once** in the file —
//!   duplicates are a typed [`ToolsError::AmbiguousAnchor`] rejection,
//!   never a first-match application (a bare `}` needs context lines to
//!   disambiguate, by design);
//! - an optional [`crate::anchors::Hashline`] adds drift detection: small
//!   external shifts re-anchor, large drift or ambiguity is a typed
//!   rejection and the file is never touched;
//! - after application the post-edit content is parsed by `austringer-ast`
//!   (syntax gate): edits that introduce NEW error/missing nodes are
//!   rejected with the diagnostic; already-broken files may be edited as
//!   long as the edit does not add errors; files without a grammar skip
//!   the gate (degrade contract);
//! - the model-facing result is the changed lines plus a ±3 fringe
//!   ([`crate::window::FRINGE_CONTEXT`]) with omission placeholders —
//!   never the whole file;
//! - structural no-rewrite guard: each edit's `new_text` is capped at
//!   [`MAX_NEW_LINES_PER_EDIT`] lines (the read-window size — you cannot
//!   write what you could never have read) and calls at
//!   [`MAX_EDITS_PER_CALL`] edits.

use crate::anchors::{AnchorStatus, Hashline};
use crate::error::ToolsError;
use crate::gate::{GateOutcome, SyntaxGate, run_gate};
use crate::lines::{LineEnding, LineTable, line_ending_bytes, logical_lines};
use crate::window::{WindowView, view_fringe};
use std::path::Path;

/// Maximum edits accepted in one call.
pub const MAX_EDITS_PER_CALL: usize = 32;

/// Maximum new lines one edit may produce (matches the read-window cap).
pub const MAX_NEW_LINES_PER_EDIT: usize = 100;

/// One anchored edit: replace `old_text` (starting at 1-based `line`) with
/// `new_text`. Insertions are `old_text` = the anchor line's content with
/// the new lines prepended in `new_text` (an empty `old_text` is a typed
/// rejection — there is no position-free insertion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEdit {
    /// 1-based anchor line.
    pub line: usize,
    /// Expected logical content at `[line, line + lines(old_text))`.
    pub old_text: String,
    /// Replacement logical content.
    pub new_text: String,
    /// Optional hashline anchor for drift detection.
    pub hash: Option<Hashline>,
}

/// Result of applying a batch of edits.
#[derive(Debug, Clone)]
pub struct EditOutcome {
    /// 1-based line numbers of every produced line, in the NEW file.
    pub changed_lines: Vec<usize>,
    /// Model-facing bounded result: changed region ±3 with omission
    /// placeholders. Never the whole file.
    pub fringe: WindowView,
    /// New content (for the writer). Not a model-facing echo.
    pub table: LineTable,
    /// Syntax-gate ruling.
    pub gate: GateOutcome,
}

struct ResolvedEdit<'a> {
    index: usize,
    line: usize,
    old_len: usize,
    edit: &'a LineEdit,
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut cut: String = text.chars().take(max).collect();
        cut.push('…');
        cut
    }
}

fn find_sequence(haystack: &[&str], needle: &[&str]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for start in 0..=(haystack.len() - needle.len()) {
        if &haystack[start..start + needle.len()] == needle {
            hits.push(start + 1);
        }
    }
    hits
}

fn anchor_line(edit: &LineEdit, table: &LineTable) -> Result<usize, ToolsError> {
    match &edit.hash {
        Some(anchor) => match anchor.verify(table) {
            AnchorStatus::Matched(line) => Ok(line),
            AnchorStatus::ReAnchored { to, .. } => Ok(to),
            AnchorStatus::Drifted { candidates } => Err(ToolsError::AnchorDrift {
                anchor_line: anchor.line,
                candidates,
            }),
        },
        None => Ok(edit.line),
    }
}

fn resolve_edits<'a>(
    table: &LineTable,
    edits: &'a [LineEdit],
) -> Result<Vec<ResolvedEdit<'a>>, ToolsError> {
    let contents = table.contents();
    let mut resolved = Vec::with_capacity(edits.len());
    for (index, edit) in edits.iter().enumerate() {
        let old = logical_lines(&edit.old_text);
        if old.is_empty() {
            return Err(ToolsError::EmptyOldText { line: edit.line });
        }
        let new = logical_lines(&edit.new_text);
        if new.len() > MAX_NEW_LINES_PER_EDIT {
            return Err(ToolsError::EditTooLarge {
                new_lines: new.len(),
                max: MAX_NEW_LINES_PER_EDIT,
            });
        }
        let line = anchor_line(edit, table)?;
        if line == 0 || line > table.line_count() {
            return Err(ToolsError::AnchorMismatch {
                line,
                expected: truncate(old[0], 60),
                actual: "<beyond end of file>".to_owned(),
            });
        }
        for (offset, expected) in old.iter().enumerate() {
            let actual = table.content_opt(line + offset);
            if actual != Some(*expected) {
                return Err(ToolsError::AnchorMismatch {
                    line: line + offset,
                    expected: truncate(expected, 60),
                    actual: truncate(actual.unwrap_or("<end of file>"), 60),
                });
            }
        }
        let occurrences = find_sequence(&contents, &old);
        if occurrences.len() > 1 {
            return Err(ToolsError::AmbiguousAnchor { line, occurrences });
        }
        resolved.push(ResolvedEdit {
            index,
            line,
            old_len: old.len(),
            edit,
        });
    }
    resolved.sort_by_key(|item| item.line);
    for pair in resolved.windows(2) {
        if pair[0].line + pair[0].old_len > pair[1].line {
            return Err(ToolsError::OverlappingEdits {
                first: pair[0].index.min(pair[1].index),
                second: pair[0].index.max(pair[1].index),
            });
        }
    }
    Ok(resolved)
}

fn apply_resolved(table: &LineTable, resolved: &[ResolvedEdit]) -> (LineTable, Vec<usize>) {
    let total = table.line_count();
    let mut out = String::with_capacity(table.as_str().len() + 256);
    let mut changed = Vec::new();
    let mut next_edit = 0;
    let mut new_line = 0usize;
    let mut line = 1usize;
    while line <= total {
        if next_edit < resolved.len() && resolved[next_edit].line == line {
            let item = &resolved[next_edit];
            let raw_ending = table.ending(item.line);
            let ending = match raw_ending {
                Some(LineEnding::Eof) | None => table.dominant_ending(),
                Some(other) => other,
            };
            let at_eof = item.line + item.old_len > total;
            let new_lines = logical_lines(&item.edit.new_text);
            let count = new_lines.len();
            for (offset, content) in new_lines.iter().enumerate() {
                new_line += 1;
                changed.push(new_line);
                out.push_str(content);
                let last_new_line = offset + 1 == count;
                if !(at_eof && last_new_line && raw_ending == Some(LineEnding::Eof)) {
                    out.push_str(line_ending_bytes(ending));
                }
            }
            line += item.old_len;
            next_edit += 1;
        } else {
            new_line += 1;
            if let Some(content) = table.content_opt(line) {
                out.push_str(content);
            }
            if let Some(ending) = table.ending(line) {
                out.push_str(line_ending_bytes(ending));
            }
            line += 1;
        }
    }
    (LineTable::from_string(out), changed)
}

/// Apply `edits` to in-memory `table` content. `gate` of `None` disables
/// the syntax gate (tool dispatch supplies [`SyntaxGate::for_path`]).
pub fn apply_edits(
    path: &Path,
    table: &LineTable,
    edits: &[LineEdit],
    gate: Option<&SyntaxGate>,
) -> Result<EditOutcome, ToolsError> {
    if edits.is_empty() {
        return Err(ToolsError::NoEdits);
    }
    if edits.len() > MAX_EDITS_PER_CALL {
        return Err(ToolsError::TooManyEdits {
            count: edits.len(),
            max: MAX_EDITS_PER_CALL,
        });
    }
    let resolved = resolve_edits(table, edits)?;
    let (new_table, changed) = apply_resolved(table, &resolved);
    let gate_outcome = run_gate(gate, table.as_bytes(), new_table.as_bytes())?;
    let focus = match (changed.first(), changed.last()) {
        (Some(first), Some(last)) => *first..=*last,
        _ => 1..=1,
    };
    Ok(EditOutcome {
        fringe: view_fringe(path, &new_table, focus),
        changed_lines: changed,
        table: new_table,
        gate: gate_outcome,
    })
}

/// Tool-level entry: read `path`, apply `edits`, run the syntax gate
/// (auto-resolved from the path when `gate` is `None`), and write the file
/// back **only** when everything above passed.
pub fn apply_edits_path(
    path: &Path,
    edits: &[LineEdit],
    gate: Option<&SyntaxGate>,
) -> Result<EditOutcome, ToolsError> {
    let bytes = std::fs::read(path)?;
    let table = LineTable::from_bytes(bytes).map_err(|_| ToolsError::NonUtf8 {
        path: path.to_path_buf(),
    })?;
    let resolved_gate;
    let gate_ref = match gate {
        Some(g) => g,
        None => {
            resolved_gate = SyntaxGate::for_path(path);
            &resolved_gate
        }
    };
    let outcome = apply_edits(path, &table, edits, Some(gate_ref))?;
    std::fs::write(path, outcome.table.as_bytes())?;
    Ok(outcome)
}
