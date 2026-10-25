//! Applying staged hunk groups selectively (plan todo 16, clause 4): the
//! y/n/e review flow over [`crate::hunks::EntityHunkGroup`]s.
//!
//! - [`EntityDecision::Yes`] applies the whole group;
//! - [`EntityDecision::No`] skips it;
//! - [`EntityDecision::Expand`] walks the group **per replacement** — the
//!   finest granularity under entity hunks (Mentat);
//!
//! every accepted change reports a ±3 fringe view of the final content
//! (bounded, with omission placeholders). An optional syntax gate runs
//! over the final content exactly like `edit_lines`.

use crate::error::ToolsError;
use crate::gate::{GateOutcome, SyntaxGate, run_gate};
use crate::hunks::{EntityHunkGroup, Replacement};
use crate::lines::{LineEnding, LineTable, line_ending_bytes};
use crate::window::{WindowView, view_fringe};
use std::path::Path;

/// Decision for one entity group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityDecision {
    /// Apply every replacement in the group.
    Yes,
    /// Skip the whole group.
    No,
    /// Decide per replacement (must match the group's replacement count).
    Expand(Vec<ReplacementDecision>),
}

/// Decision for one replacement inside an expanded group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementDecision {
    /// Apply this replacement.
    Yes,
    /// Skip this replacement.
    No,
}

/// Result of a selective application.
#[derive(Debug, Clone)]
pub struct StagedOutcome {
    /// Final content (for the writer).
    pub table: LineTable,
    /// 1-based changed lines in the final content.
    pub changed_lines: Vec<usize>,
    /// One bounded fringe view per accepted change.
    pub fringes: Vec<WindowView>,
    /// Syntax-gate ruling over the final content.
    pub gate: GateOutcome,
}

struct Accepted {
    replacement: Replacement,
    new_contents: Vec<String>,
}

fn new_contents_of(new: &LineTable, replacement: Replacement) -> Vec<String> {
    if replacement.new_end == 0 || replacement.new_end < replacement.new_start {
        Vec::new()
    } else {
        (replacement.new_start..=replacement.new_end)
            .filter_map(|line| new.content_opt(line).map(str::to_owned))
            .collect()
    }
}

fn gather(
    groups: &[EntityHunkGroup],
    decisions: &[EntityDecision],
    new: &LineTable,
) -> Result<Vec<Accepted>, ToolsError> {
    if decisions.len() != groups.len() {
        return Err(ToolsError::InvalidDecisions {
            group: groups.len(),
            expected: groups.len(),
            got: decisions.len(),
        });
    }
    let mut accepted = Vec::new();
    for (index, (group, decision)) in groups.iter().zip(decisions).enumerate() {
        let chosen: Vec<Replacement> = match decision {
            EntityDecision::Yes => group.replacements.clone(),
            EntityDecision::No => Vec::new(),
            EntityDecision::Expand(per) => {
                if per.len() != group.replacements.len() {
                    return Err(ToolsError::InvalidDecisions {
                        group: index,
                        expected: group.replacements.len(),
                        got: per.len(),
                    });
                }
                group
                    .replacements
                    .iter()
                    .zip(per)
                    .filter(|(_, decision)| **decision == ReplacementDecision::Yes)
                    .map(|(replacement, _)| *replacement)
                    .collect()
            }
        };
        accepted.extend(chosen.into_iter().map(|replacement| Accepted {
            new_contents: new_contents_of(new, replacement),
            replacement,
        }));
    }
    accepted.sort_by_key(|item| (item.replacement.old_start, item.replacement.old_end));
    Ok(accepted)
}

/// One emitted region: lines `[first, last]` in the final content.
#[derive(Debug, Clone, Copy)]
struct Emitted {
    first: usize,
    last: usize,
}

struct Builder<'a> {
    out: String,
    new_line: usize,
    changed: Vec<usize>,
    emitted: Vec<Emitted>,
    accepted: &'a [Accepted],
    next: usize,
}

impl Builder<'_> {
    fn push_content(&mut self, content: &str, ending: &str) {
        self.new_line += 1;
        self.changed.push(self.new_line);
        self.out.push_str(content);
        self.out.push_str(ending);
    }

    fn drain_insertions(&mut self, anchor: usize, old: &LineTable) {
        while self.next < self.accepted.len()
            && self.accepted[self.next].replacement.old_end == 0
            && self.accepted[self.next].replacement.old_start == anchor
        {
            let reference_line = if anchor == 0 { 1 } else { anchor };
            let ending = match old.ending(reference_line) {
                Some(LineEnding::Eof) | None => old.dominant_ending(),
                Some(other) => other,
            };
            let first = self.new_line + 1;
            let count = self.accepted[self.next].new_contents.len();
            for content in &self.accepted[self.next].new_contents {
                self.push_content(content, line_ending_bytes(ending));
            }
            if count > 0 {
                self.emitted.push(Emitted {
                    first,
                    last: self.new_line,
                });
            }
            self.next += 1;
        }
    }
}

/// Apply the accepted parts of `groups` (staged against `new`) to `old`,
/// producing bounded per-change fringes and (optionally) running the
/// syntax gate.
pub fn apply_staged(
    path: &Path,
    old: &LineTable,
    new: &LineTable,
    groups: &[EntityHunkGroup],
    decisions: &[EntityDecision],
    gate: Option<&SyntaxGate>,
) -> Result<StagedOutcome, ToolsError> {
    let accepted = gather(groups, decisions, new)?;
    let total = old.line_count();
    let mut builder = Builder {
        out: String::with_capacity(old.as_str().len() + 256),
        new_line: 0,
        changed: Vec::new(),
        emitted: Vec::new(),
        accepted: &accepted,
        next: 0,
    };
    builder.drain_insertions(0, old);
    let mut line = 1usize;
    while line <= total {
        let is_replacement = builder.next < accepted.len()
            && accepted[builder.next].replacement.old_start == line
            && accepted[builder.next].replacement.old_end != 0;
        if is_replacement {
            let item = &accepted[builder.next];
            let ending = match old.ending(line) {
                Some(LineEnding::Eof) | None => old.dominant_ending(),
                Some(other) => other,
            };
            let raw_eof = old.ending(item.replacement.old_end) == Some(LineEnding::Eof);
            let covers_eof = item.replacement.old_end >= total;
            let first = builder.new_line + 1;
            let count = item.new_contents.len();
            for (offset, content) in item.new_contents.iter().enumerate() {
                let is_last = offset + 1 == count;
                let terminator = if covers_eof && is_last && raw_eof {
                    ""
                } else {
                    line_ending_bytes(ending)
                };
                builder.push_content(content, terminator);
            }
            builder.emitted.push(Emitted {
                first,
                last: builder.new_line,
            });
            let replaced_through = item.replacement.old_end;
            builder.next += 1;
            builder.drain_insertions(replaced_through, old);
            line = replaced_through + 1;
        } else {
            let content = old.content_opt(line).unwrap_or_default();
            let terminator = old.ending(line).map_or("", line_ending_bytes);
            builder.new_line += 1;
            builder.out.push_str(content);
            builder.out.push_str(terminator);
            builder.drain_insertions(line, old);
            line += 1;
        }
    }

    let table = LineTable::from_string(builder.out);
    let gate_outcome = run_gate(gate, old.as_bytes(), table.as_bytes())?;
    let fringes = builder
        .emitted
        .iter()
        .map(|region| view_fringe(path, &table, region.first..=region.last))
        .collect();
    Ok(StagedOutcome {
        table,
        changed_lines: builder.changed,
        fringes,
        gate: gate_outcome,
    })
}
