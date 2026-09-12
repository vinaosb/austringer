//! Bounded file viewing (plan todo 16, clauses 1 + 10):
//! - windows of at most [`MAX_WINDOW_LINES`] lines, 1-based, with
//!   jump-to-line (`start_line`);
//! - a context header (path, window range, total lines) on every view, so
//!   oversized files are never mistaken for whole files;
//! - the **anti-elision guard**: lines outside the window are represented
//!   by explicit `[... N lines omitted ...]` placeholders the model is
//!   trained to notice — relevant content is never silently dropped.
//!
//! No API here returns or accepts a whole file body for editing (clause 12);
//! views are the only file-shaped output, and they are windowed.

use crate::error::ToolsError;
use crate::lines::LineTable;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

/// Maximum lines shown in one window (plan: ≤100).
pub const MAX_WINDOW_LINES: usize = 100;

/// Fringe context around changed lines in edit results (plan: ±3).
pub const FRINGE_CONTEXT: usize = 3;

/// The omission placeholder emitted for every elided region.
fn omitted_marker(count: usize) -> String {
    format!("[... {count} lines omitted ...]")
}

/// A rendered, bounded view of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowView {
    /// Context header line (`path (lines a-b of n)`), present on every view.
    pub header: String,
    /// Numbered body lines (and omission placeholders) as one string.
    pub body: String,
    /// First 1-based line shown.
    pub first_line: usize,
    /// Last 1-based line shown.
    pub last_line: usize,
    /// Total lines in the file.
    pub total_lines: usize,
    /// Lines above the window (reported by a placeholder).
    pub omitted_before: usize,
    /// Lines below the window (reported by a placeholder).
    pub omitted_after: usize,
}

impl WindowView {
    /// Header + body as one block.
    pub fn render(&self) -> String {
        format!("{}\n{}", self.header, self.body)
    }
}

/// Render a window view (shared by windows, fringes, and block reads).
pub fn build_window_view(
    path: &Path,
    table: &LineTable,
    window: RangeInclusive<usize>,
) -> WindowView {
    let total = table.line_count();
    if total == 0 {
        return WindowView {
            header: format!("{} (empty file)", path.display()),
            body: "(empty file)".to_owned(),
            first_line: 0,
            last_line: 0,
            total_lines: 0,
            omitted_before: 0,
            omitted_after: 0,
        };
    }
    let first = *window.start();
    let last = (*window.end()).min(total);
    let width = total.to_string().len();
    let mut body = String::new();
    if first > 1 {
        body.push_str(&omitted_marker(first - 1));
        body.push('\n');
    }
    for line in window {
        if let Some(content) = table.content_opt(line) {
            body.push_str(&format!("{line:>width$} | {content}\n"));
        }
    }
    if last < total {
        body.push_str(&omitted_marker(total - last));
        body.push('\n');
    }
    WindowView {
        header: format!("{} (lines {first}-{last} of {total})", path.display()),
        body,
        first_line: first,
        last_line: last,
        total_lines: total,
        omitted_before: first - 1,
        omitted_after: total - last,
    }
}

fn resolve_window(
    total: usize,
    start_line: Option<usize>,
) -> Result<RangeInclusive<usize>, ToolsError> {
    if total == 0 {
        return Ok(1..=1);
    }
    let first = match start_line {
        Some(line) if line == 0 || line > total => {
            return Err(ToolsError::LineOutOfRange { line, total });
        }
        other => other.unwrap_or(1),
    };
    let last = (first + MAX_WINDOW_LINES - 1).min(total);
    Ok(first..=last)
}

/// Render a bounded window of `source` bytes of `path`.
///
/// Errors: [`ToolsError::NonUtf8`], [`ToolsError::LineOutOfRange`] when
/// `start_line` is 0 or beyond the file.
pub fn view_window(
    path: &Path,
    source: &[u8],
    start_line: Option<usize>,
) -> Result<WindowView, ToolsError> {
    let table = LineTable::from_bytes(source.to_vec())?;
    let window = resolve_window(table.line_count(), start_line)?;
    Ok(build_window_view(path, &table, window))
}

/// Read `path` from disk and render a bounded window of it.
pub fn view_window_path(path: &Path, start_line: Option<usize>) -> Result<WindowView, ToolsError> {
    let bytes = std::fs::read(path)?;
    view_window(path, &bytes, start_line)
}

/// Render the fringe view around a changed region of an (already parsed)
/// table: lines `focus ± FRINGE_CONTEXT`, with the same context header and
/// omission placeholders as [`view_window`]. This is the bounded result
/// shape edit tools return — changed lines plus context, never the file.
pub fn view_fringe(path: &Path, table: &LineTable, focus: RangeInclusive<usize>) -> WindowView {
    let total = table.line_count();
    let (first, last) = if total == 0 {
        (1, 1)
    } else {
        let center_lo = (*focus.start()).clamp(1, total);
        let center_hi = (*focus.end()).clamp(center_lo, total);
        (
            center_lo.saturating_sub(FRINGE_CONTEXT).max(1),
            (center_hi + FRINGE_CONTEXT).min(total),
        )
    };
    build_window_view(path, table, first..=last)
}

/// Path used by tool results when content is in-memory (`-`).
pub fn memory_path() -> PathBuf {
    PathBuf::from("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fringe_clamps_to_file_bounds() {
        let table = LineTable::from_string("1\n2\n3\n4\n5\n".to_owned());
        let view = view_fringe(Path::new("f"), &table, 4..=4);
        assert_eq!((view.first_line, view.last_line), (1, 5));
        let view = view_fringe(Path::new("f"), &table, 1..=2);
        assert_eq!((view.first_line, view.last_line), (1, 5));
    }

    #[test]
    fn fringe_marks_outside_lines_as_omitted() {
        let body: String = (1..=50).map(|n| format!("l{n}\n")).collect();
        let table = LineTable::from_string(body);
        let view = view_fringe(Path::new("f"), &table, 25..=25);
        assert_eq!(view.omitted_before, 21);
        assert_eq!(view.omitted_after, 22);
        assert!(view.body.contains("[... 21 lines omitted ...]"));
        assert!(view.body.contains("[... 22 lines omitted ...]"));
    }
}
