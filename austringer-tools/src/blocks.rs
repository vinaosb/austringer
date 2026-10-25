//! Indentation-anchored block reads (plan todo 16, clause 5 — Roo
//! pattern): `anchor_line` expands to the whole semantic block around it,
//! a cheap targeted-retrieval primitive beyond line windows.
//!
//! Expansion rules (language-neutral, indentation-based):
//! - anchor on a **container start** (the next non-blank line below is
//!   indented deeper) → block = that line through its body;
//! - anchor inside a body → expand up to the nearest non-blank line with
//!   smaller indentation (the container header), then down to the end of
//!   that container;
//! - block end stops at the first non-blank line indented no deeper than
//!   the header (a `}`-style closer at header level is included);
//! - trailing blank lines are not part of a block;
//! - blank anchors are a typed rejection; oversized blocks render as a
//!   window with omission placeholders (never silent truncation).
//!
//! Indentation width counts a tab as advancing to the next multiple of 4.

use crate::error::ToolsError;
use crate::lines::LineTable;
use crate::window::{MAX_WINDOW_LINES, build_window_view};
use std::path::Path;

/// The expanded semantic block around an anchor line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockView {
    /// First 1-based line of the block.
    pub first_line: usize,
    /// Last 1-based line of the block (trailing blanks trimmed).
    pub last_line: usize,
    /// Lines in the file (for orientation).
    pub total_lines: usize,
    /// The bounded rendering (windowed with placeholders when the block
    /// exceeds [`MAX_WINDOW_LINES`] lines).
    pub view: crate::window::WindowView,
}

fn is_blank(content: &str) -> bool {
    content.trim().is_empty()
}

fn indent_width(content: &str) -> usize {
    let mut width = 0;
    for ch in content.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width = (width / 4 + 1) * 4,
            _ => break,
        }
    }
    width
}

fn next_non_blank_below(table: &LineTable, line: usize) -> Option<(usize, usize)> {
    let mut probe = line + 1;
    while probe <= table.line_count() {
        let content = table.content_opt(probe)?;
        if !is_blank(content) {
            return Some((probe, indent_width(content)));
        }
        probe += 1;
    }
    None
}

/// Nearest non-blank line above `line` with indent strictly less than
/// `base`; blank lines are skipped but never included.
fn header_above(table: &LineTable, line: usize, base: usize) -> Option<usize> {
    let mut probe = line.saturating_sub(1);
    while probe >= 1 {
        let content = table.content_opt(probe)?;
        if !is_blank(content) && indent_width(content) < base {
            return Some(probe);
        }
        probe -= 1;
    }
    None
}

fn block_end(table: &LineTable, header: usize, header_indent: usize) -> usize {
    let mut end = header;
    let mut probe = header + 1;
    while probe <= table.line_count() {
        let content = table.content_opt(probe).unwrap_or_default();
        if is_blank(content) {
            let deeper_below = next_non_blank_below(table, probe)
                .map(|(_, indent)| indent > header_indent)
                .unwrap_or(false);
            if deeper_below {
                probe += 1;
                continue;
            }
            break;
        }
        if indent_width(content) > header_indent {
            end = probe;
            probe += 1;
            continue;
        }
        let trimmed = content.trim_end();
        if (trimmed == "}" || trimmed == "};") && indent_width(content) == header_indent {
            end = probe;
        }
        break;
    }
    end
}

/// Expand `anchor` (1-based) to its semantic block and render it.
pub fn read_block(path: &Path, table: &LineTable, anchor: usize) -> Result<BlockView, ToolsError> {
    let total = table.line_count();
    if anchor == 0 || anchor > total {
        return Err(ToolsError::LineOutOfRange {
            line: anchor,
            total,
        });
    }
    let anchor_content = table.content_opt(anchor).unwrap_or_default();
    if is_blank(anchor_content) {
        return Err(ToolsError::BlankAnchor { line: anchor });
    }
    let base = indent_width(anchor_content);
    let header = match next_non_blank_below(table, anchor) {
        Some((_, below_indent)) if below_indent > base => anchor,
        _ => header_above(table, anchor, base).unwrap_or(anchor),
    };
    let header_indent = indent_width(table.content_opt(header).unwrap_or_default());
    let end = block_end(table, header, header_indent);
    let view_last = (header + MAX_WINDOW_LINES - 1).min(end);
    Ok(BlockView {
        first_line: header,
        last_line: end,
        total_lines: total,
        view: build_window_view(path, table, header..=view_last),
    })
}
