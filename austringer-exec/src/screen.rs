//! The screen-delta terminal model (plan todo 19).
//!
//! PTY bytes are fed through a [`vt100::Parser`] grid — the canonical
//! model is the screen those bytes paint, never the byte stream:
//! ANSI/SGR never survives into [`ScreenView`] (plain cell text only),
//! carriage-return rewrites collapse to the final line, invalid UTF-8 is
//! skipped by the parser (typed skip, never a panic), and rows scrolling
//! off the top land in a bounded [`Transcript`]
//! (head + tail + count), so a 100 MB stream costs O(115) rows of memory.
//!
//! # Slice discipline
//!
//! [`ScreenModel::process`] never feeds a whole chunk blindly: input is
//! cut into slices at every scroll-capable boundary (printable runs
//! bounded by the wrap budget, `\n`/`\v`/`\f`, escape sequences). Between
//! slices the emulator decides cheaply whether a scroll happened and
//! captures the lost top row in O(1); anything ambiguous (escape
//! sequences, cursor jumps, resize) falls back to a full snapshot diff
//! with shift detection. Two fast paths keep bulk line output linear:
//! a `\n` on the bottom row confirms a one-row scroll by comparing the
//! new top row, and a printable run can only scroll via the single
//! wrap byte at the last column.
//!
//! Known attribution limit (documented): a scroll-region `\n` mid-screen
//! with an intact top row shifts middle rows without notice; later
//! captures could attribute stale rows. Only reachable by apps that use
//! scroll regions without the alternate screen; full-screen TUIs are
//! handled exactly — while `alternate_screen` is active, transcript
//! capture is suspended entirely (alternate screens have no scrollback
//! semantics). The raw stream in the spill file remains the lossless
//! record either way.

use std::collections::VecDeque;

use crate::transcript::Transcript;

/// Bounded view over one command's terminal output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenView {
    /// Final visible screen lines (logical, wrapped rows joined).
    pub visible: Vec<String>,
    /// Bounded scrolled-row excerpt (first [`crate::window::HEAD_LINES`],
    /// then tail rows; see [`Transcript::composed`]).
    pub scrolled: Vec<String>,
    /// Total rows that scrolled off the top.
    pub scrolled_total: u64,
    /// Redraw-attribution ambiguity was detected.
    pub lossy: bool,
}

impl ScreenView {
    /// The bounded transcript lines (scrolled excerpt + visible screen),
    /// ready for [`crate::window::window_lines`].
    #[must_use]
    pub fn transcript_lines(&self) -> Vec<String> {
        let mut lines = self.scrolled.clone();
        lines.extend(self.visible.iter().cloned());
        lines
    }
}

/// Screen emulator fed with PTY byte chunks.
pub struct ScreenModel {
    parser: vt100::Parser,
    cols: u16,
    rows: u16,
    transcript: Transcript,
    prev: VecDeque<String>,
    cursor: (u16, u16),
    alt_screen: bool,
}

impl ScreenModel {
    /// A blank screen of the given geometry. Degenerate sizes clamp to
    /// 1x1 so the slice arithmetic stays panic-free.
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        let (cols, rows) = (cols.max(1), rows.max(1));
        let mut model = Self {
            parser: vt100::Parser::new(rows, cols, 0),
            cols,
            rows,
            transcript: Transcript::new(),
            prev: VecDeque::new(),
            cursor: (0, 0),
            alt_screen: false,
        };
        model.reset_prev();
        model
    }

    /// Feeds one chunk of PTY bytes, sliced at scroll-capable boundaries.
    pub fn process(&mut self, chunk: &[u8]) {
        let mut i = 0;
        while i < chunk.len() {
            let byte = chunk[i];
            let printable = |b: u8| (b >= 0x20 && b != 0x7f) || b >= 0x80;
            if printable(byte) {
                let budget = (self.cols - self.cursor.1.min(self.cols - 1)).max(1) as usize;
                let mut end = i;
                while end < chunk.len() && end - i < budget && printable(chunk[end]) {
                    end += 1;
                }
                self.feed_printable(&chunk[i..end]);
                i = end;
            } else if byte == 0x1b {
                let mut end = i + 1;
                while end < chunk.len() {
                    let b = chunk[end];
                    // `[`/`]` immediately after ESC introduce CSI/OSC and
                    // are never the final byte.
                    let introducer = end == i + 1 && matches!(b, b'[' | b']');
                    if (0x40..=0x7e).contains(&b) && !introducer {
                        end += 1;
                        break;
                    }
                    if b < 0x20 {
                        break;
                    }
                    end += 1;
                }
                self.feed_escape(&chunk[i..end]);
                i = end;
            } else if matches!(byte, b'\n' | b'\x0b' | b'\x0c') {
                self.feed_newline();
                i += 1;
            } else {
                self.feed_control(byte);
                i += 1;
            }
        }
    }

    /// Resizes the grid mid-stream (bounded reflow).
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.parser.screen_mut().set_size(rows, cols);
        self.cols = cols;
        self.rows = rows;
        self.reset_prev();
        self.cursor = self.parser.screen().cursor_position();
    }

    /// Current cursor position (row, col) straight from the grid.
    #[must_use]
    pub fn cursor(&self) -> (u16, u16) {
        self.parser.screen().cursor_position()
    }

    /// Consume the model, yielding the bounded view.
    #[must_use]
    pub fn finish(self) -> ScreenView {
        let visible = self.visible_lines();
        ScreenView {
            visible,
            scrolled: self.transcript.composed(),
            scrolled_total: self.transcript.total(),
            lossy: self.transcript.is_lossy(),
        }
    }

    /// Final visible screen as logical lines (trailing cell padding
    /// trimmed; wrapped rows joined by the grid renderer).
    #[must_use]
    pub fn visible_lines(&self) -> Vec<String> {
        let contents = self.parser.screen().contents();
        if contents.is_empty() {
            Vec::new()
        } else {
            contents
                .lines()
                .map(|line| line.trim_end().to_string())
                .collect()
        }
    }

    fn reset_prev(&mut self) {
        self.prev = (0..self.rows).map(|row| self.row_text(row)).collect();
    }

    fn row_text(&self, row: u16) -> String {
        let screen = self.parser.screen();
        let mut text = String::with_capacity(self.cols as usize);
        for col in 0..self.cols {
            if let Some(cell) = screen.cell(row, col) {
                if cell.is_wide_continuation() {
                    continue;
                }
                text.push_str(cell.contents());
            }
        }
        text.trim_end().to_string()
    }

    fn feed_printable(&mut self, run: &[u8]) {
        let before = self.cursor;
        let last_row = self.rows - 1;
        self.parser.process(run);
        let after = self.parser.screen().cursor_position();
        if after.0 == before.0 {
            // Budget guarantees no wrap within the run.
            self.sync_row(before.0);
            self.cursor = after;
        } else if after.0 == before.0 + 1 && after.0 <= last_row {
            // Wrapped one row down; wrapping INTO the last row cannot
            // scroll (scroll happens only when wrapping FROM it).
            self.sync_row(before.0);
            self.sync_row(after.0);
            self.cursor = after;
        } else if before.0 == last_row && after.0 == last_row && after.1 <= 1 && run.len() == 1 {
            // The wrap byte at the bottom-right corner scrolled one row.
            let lost = self.prev.pop_front().unwrap_or_default();
            self.transcript.capture(lost);
            self.prev.push_back(self.row_text(last_row));
            self.cursor = after;
        } else {
            self.resync();
        }
    }

    fn feed_newline(&mut self) {
        let before_row = self.cursor.0;
        self.parser.process(b"\n");
        if self.alt_screen {
            self.cursor = self.parser.screen().cursor_position();
            return;
        }
        let new_top = self.row_text(0);
        let scrolled_confirmable = self.prev.len() > 1 && new_top == self.prev[1];
        let top_intact = new_top == self.prev[0];
        if before_row == self.rows - 1 {
            if scrolled_confirmable {
                let lost = self.prev.pop_front().unwrap_or_default();
                self.transcript.capture(lost);
                self.prev.push_back(self.row_text(self.rows - 1));
            } else {
                self.resync();
            }
        } else if !top_intact {
            self.resync();
        }
        self.cursor = self.parser.screen().cursor_position();
    }

    fn feed_control(&mut self, byte: u8) {
        let before_row = self.cursor.0;
        self.parser.process(&[byte]);
        let after = self.parser.screen().cursor_position();
        if after.0 != before_row {
            self.resync();
        } else {
            self.cursor = after;
        }
    }

    fn feed_escape(&mut self, slice: &[u8]) {
        self.parser.process(slice);
        let alt = self.parser.screen().alternate_screen();
        if alt != self.alt_screen {
            self.alt_screen = alt;
            self.reset_prev();
            self.cursor = self.parser.screen().cursor_position();
            return;
        }
        self.resync();
    }

    /// Full snapshot diff: attribute any upward shift to scrolling and
    /// capture the lost rows; otherwise treat the transition as a redraw.
    fn resync(&mut self) {
        let current: Vec<String> = (0..self.rows).map(|row| self.row_text(row)).collect();
        self.cursor = self.parser.screen().cursor_position();
        if self.alt_screen {
            self.prev = current.into();
            return;
        }
        let rows = self.rows as usize;
        let mut shift = 0;
        'shifts: for candidate in 1..rows {
            let overlap = self.prev.range(candidate..);
            let shifted = overlap.zip(current.iter());
            for (previous, now) in shifted {
                if previous != now {
                    continue 'shifts;
                }
            }
            // An all-blank overlap confirms nothing: blank rows match any
            // blank rows, so a clear (2J) or blank redraw must not count
            // as a scroll of blank content.
            if (candidate..rows).any(|r| !self.prev[r].is_empty()) {
                shift = candidate;
                break;
            }
        }
        if shift > 0 {
            for _ in 0..shift {
                let lost = self.prev.pop_front().unwrap_or_default();
                self.transcript.capture(lost);
            }
        } else if self.prev.front().is_some_and(|top| !top.is_empty())
            && current.first() != self.prev.front()
            && current.iter().any(|row| !row.is_empty())
        {
            // Content vanished from the top without a confirmable shift.
            // A fully blank screen is a clear (2J), which erases rather
            // than scrolls — not a loss.
            self.transcript.mark_lossy();
        }
        self.prev = current.into();
    }

    fn sync_row(&mut self, row: u16) {
        let text = self.row_text(row);
        if let Some(slot) = self.prev.get_mut(row as usize) {
            *slot = text;
        }
    }
}
