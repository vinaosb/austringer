//! Semantic hunk staging (plan todo 16, clause 4 — Mentat pattern): an
//! imara-diff **Histogram** line diff over logical (terminator-free, so
//! CRLF-insensitive) lines, grouped by tree-sitter entities.
//!
//! The acceptance property: staged hunks **never span def/class
//! boundaries**. A raw replacement whose old range crosses entity
//! boundaries (or enters/leaves top-level space) is split into maximal
//! same-owner runs; its new lines are allocated to pieces by a mini
//! Histogram alignment inside the hunk (equal lines follow their old
//! owner; genuinely changed regions attach to the piece of their first
//! old line, split proportionally when a changed region itself crosses).
//! Cross-boundary rewrites are inherently ambiguous — the decomposition
//! is deterministic and documented, and rare by construction (entity
//! boundaries are usually equal-context anchors the diff keeps).

use crate::difftokens::histogram_hunks;
use crate::lines::LineTable;
use harness_ast::tags::Definition;

/// A tree-sitter entity as a 1-based inclusive line span (def/class/...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySpan {
    /// Entity name.
    pub name: String,
    /// Kind label (outline vocabulary: `fn`, `struct`, ...).
    pub kind: String,
    /// First line of the entity.
    pub first_line: usize,
    /// Last line of the entity.
    pub last_line: usize,
}

/// One staged replacement: old lines `[old_start, old_end]` (1-based,
/// inclusive) become new lines `[new_start, new_end]` of the staged
/// content. `old_end == 0` marks a pure insertion after `old_start`
/// (`old_start == 0` inserts at the head of the file); `new_end == 0`
/// marks a pure deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Replacement {
    /// First replaced old line (insertion anchor when `old_end == 0`).
    pub old_start: usize,
    /// Last replaced old line; `0` for pure insertions.
    pub old_end: usize,
    /// First new line in the staged new table (`0` when none).
    pub new_start: usize,
    /// Last new line in the staged new table (`0` when none).
    pub new_end: usize,
}

/// A review group: all replacements inside one entity (or in top-level
/// space, `entity == None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityHunkGroup {
    /// The owning entity, or `None` for top-level space.
    pub entity: Option<EntitySpan>,
    /// Replacements, position-ordered.
    pub replacements: Vec<Replacement>,
}

fn kind_word(kind: &harness_ast::tags::SymbolKind) -> &'static str {
    crate::outline::kind_label(kind)
}

/// Build entity spans from a tag table's definitions: `def_span` when the
/// query captured it (name span alone otherwise), sorted and merged so
/// overlapping/nested spans form one group labeled by the outermost name.
#[must_use]
pub fn entities_from_defs(table: &LineTable, defs: &[Definition]) -> Vec<EntitySpan> {
    let mut spans: Vec<EntitySpan> = defs
        .iter()
        .map(|def| {
            let (first_line, last_line) = match def.def_span {
                Some(def_span) => (
                    table.line_of_byte(def_span.start_byte),
                    table.line_of_byte(def_span.end_byte.max(def_span.start_byte + 1) - 1),
                ),
                None => {
                    let line = table.line_of_byte(def.name_span.start_byte);
                    (line, line)
                }
            };
            EntitySpan {
                name: def.name.clone(),
                kind: kind_word(&def.kind).to_owned(),
                first_line,
                last_line,
            }
        })
        .collect();
    spans.sort_by_key(|span| (span.first_line, span.last_line));
    let mut merged: Vec<EntitySpan> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.first_line <= last.last_line + 1 => {
                if span.last_line > last.last_line {
                    last.last_line = span.last_line;
                }
            }
            _ => merged.push(span),
        }
    }
    merged
}

fn owner_of(spans: &[EntitySpan], line: usize) -> Option<usize> {
    spans
        .iter()
        .position(|span| line >= span.first_line && line <= span.last_line)
}

/// Split `replacement` into maximal same-owner runs over its old lines
/// and allocate new lines to pieces via a mini alignment. Returns
/// `(owner, Replacement)` pairs.
fn split_by_owners(
    spans: &[EntitySpan],
    replacement: Replacement,
    old: &[&str],
    new: &[&str],
) -> Vec<(Option<usize>, Replacement)> {
    if replacement.old_end == 0 {
        let owner = if replacement.old_start == 0 {
            None
        } else {
            owner_of(spans, replacement.old_start)
        };
        return vec![(owner, replacement)];
    }
    let mut runs: Vec<(Option<usize>, usize, usize)> = Vec::new();
    for line in replacement.old_start..=replacement.old_end {
        let owner = owner_of(spans, line);
        match runs.last_mut() {
            Some((last_owner, _, last_end)) if *last_owner == owner => *last_end = line,
            _ => runs.push((owner, line, line)),
        }
    }
    if runs.len() == 1 {
        return vec![(runs[0].0, replacement)];
    }
    let old_slice = &old[replacement.old_start - 1..replacement.old_end];
    let new_slice: Vec<&str> = if replacement.new_end >= replacement.new_start {
        new[replacement.new_start - 1..replacement.new_end].to_vec()
    } else {
        Vec::new()
    };
    let owners = allocate_owners(old_slice, &new_slice, &runs);
    let mut out = Vec::with_capacity(runs.len());
    let mut new_next = replacement.new_start;
    for (index, (owner, first, last)) in runs.into_iter().enumerate() {
        let count = owners.iter().filter(|&&piece| piece == index).count();
        let piece_new_start = new_next;
        let piece_new_end = if count == 0 { 0 } else { new_next + count - 1 };
        if count > 0 {
            new_next += count;
        }
        out.push((
            owner,
            Replacement {
                old_start: first,
                old_end: last,
                new_start: piece_new_start,
                new_end: piece_new_end,
            },
        ));
    }
    out
}

/// Which piece each new line belongs to: equal lines follow their old
/// line's piece; changed regions attach to the piece of their first old
/// line (split proportionally when the region spans pieces).
fn allocate_owners(
    old: &[&str],
    new: &[&str],
    runs: &[(Option<usize>, usize, usize)],
) -> Vec<usize> {
    let piece_of_local = |index: usize| -> usize {
        let line = index + 1;
        runs.iter()
            .position(|(_, first, last)| line >= *first && line <= *last)
            .unwrap_or(runs.len().saturating_sub(1))
    };
    let mut owners = vec![0usize; new.len()];
    let mut i_old = 0usize;
    let mut i_new = 0usize;
    for hunk in histogram_hunks(old, new) {
        let (o0, o1) = (hunk.before.start as usize, hunk.before.end as usize);
        let (n0, n1) = (hunk.after.start as usize, hunk.after.end as usize);
        while i_old < o0 && i_new < n0 {
            owners[i_new] = piece_of_local(i_old);
            i_old += 1;
            i_new += 1;
        }
        i_old = i_old.max(o0);
        i_new = i_new.max(n0);
        if o1 > o0 {
            let first_piece = piece_of_local(o0);
            let total = n1 - n0;
            let mut assigned = 0usize;
            for piece in first_piece..runs.len() {
                if assigned >= total {
                    break;
                }
                let piece_old = runs[piece].2 - runs[piece].1 + 1;
                let take = if piece == runs.len() - 1 {
                    total - assigned
                } else {
                    (total * piece_old / (o1 - o0).max(1)).min(total - assigned)
                };
                for slot in &mut owners[i_new + assigned..i_new + assigned + take] {
                    *slot = piece;
                }
                assigned += take;
            }
        } else if n1 > n0 {
            let piece = if i_old == 0 {
                0
            } else {
                piece_of_local(i_old - 1)
            };
            for slot in &mut owners[n0..n1] {
                *slot = piece;
            }
        }
        i_old = i_old.max(o1);
        i_new = i_new.max(n1);
    }
    while i_old < old.len() && i_new < new.len() {
        owners[i_new] = piece_of_local(i_old);
        i_old += 1;
        i_new += 1;
    }
    owners
}

/// Stage the Histogram diff of `old` → `new`, grouped by `spans`.
#[must_use]
pub fn stage_hunks(old: &LineTable, new: &LineTable, spans: &[EntitySpan]) -> Vec<EntityHunkGroup> {
    let old_lines = old.contents();
    let new_lines = new.contents();
    let mut groups: Vec<EntityHunkGroup> = Vec::new();
    for hunk in histogram_hunks(&old_lines, &new_lines) {
        let old_start = if hunk.before.is_empty() {
            hunk.before.start as usize
        } else {
            hunk.before.start as usize + 1
        };
        let old_end = if hunk.before.is_empty() {
            0
        } else {
            hunk.before.end as usize
        };
        let new_len = hunk.after.end - hunk.after.start;
        let new_start = if new_len == 0 {
            0
        } else {
            hunk.after.start as usize + 1
        };
        let new_end = if new_len == 0 {
            0
        } else {
            hunk.after.start as usize + new_len as usize
        };
        let replacement = Replacement {
            old_start,
            old_end,
            new_start,
            new_end,
        };
        for (owner, piece) in split_by_owners(spans, replacement, &old_lines, &new_lines) {
            let group_index = match owner {
                Some(index) => groups
                    .iter()
                    .position(|group| group.entity.as_ref() == Some(&spans[index])),
                None => groups.iter().position(|group| group.entity.is_none()),
            };
            let index = match group_index {
                Some(existing) => existing,
                None => {
                    groups.push(EntityHunkGroup {
                        entity: owner.map(|index| spans[index].clone()),
                        replacements: Vec::new(),
                    });
                    groups.len() - 1
                }
            };
            groups[index].replacements.push(piece);
        }
    }
    groups
}
