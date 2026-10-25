//! Bounded scrollback transcript (head + tail + count).
//!
//! The screen-delta model never retains the full scrollback: rows that
//! scroll off the top are captured into a fixed-size head and tail with a
//! monotonic counter, so memory stays O([`HEAD_LINES`] + [`TAIL_LINES`])
//! rows no matter how much output a command produces.
//! [`Transcript::composed`] yields the bounded excerpt the window law
//! renders: the head, and — only when the full scroll does not fit — the
//! tail, with the overlap arithmetic for in-between totals.

use std::collections::VecDeque;

use crate::window::{HEAD_LINES, TAIL_LINES, WINDOW_LIMIT};

/// Captures scrolled-off rows: first [`HEAD_LINES`], last [`TAIL_LINES`],
/// and the total count.
#[derive(Debug)]
pub(crate) struct Transcript {
    head: Vec<String>,
    tail: VecDeque<String>,
    total: u64,
    lossy: bool,
}

impl Transcript {
    /// An empty transcript.
    pub(crate) fn new() -> Self {
        Self {
            head: Vec::new(),
            tail: VecDeque::new(),
            total: 0,
            lossy: false,
        }
    }

    /// Records one scrolled-off row.
    pub(crate) fn capture(&mut self, row: String) {
        if self.head.len() < HEAD_LINES {
            self.head.push(row.clone());
        }
        if self.tail.len() == TAIL_LINES {
            self.tail.pop_front();
        }
        self.tail.push_back(row);
        self.total += 1;
    }

    /// Flags that at least one screen transition could not be attributed
    /// (possible unrecorded scroll during a redraw).
    pub(crate) fn mark_lossy(&mut self) {
        self.lossy = true;
    }

    /// Rows captured so far.
    pub(crate) fn total(&self) -> u64 {
        self.total
    }

    /// Whether attribution was ever ambiguous.
    pub(crate) fn is_lossy(&self) -> bool {
        self.lossy
    }

    /// The bounded scrolled-row excerpt for windowing.
    ///
    /// Up to [`WINDOW_LIMIT`] scrolled rows the excerpt is complete
    /// (head plus the non-overlapping tail remainder); beyond that it is
    /// head + tail and the window's omission marker names the difference.
    pub(crate) fn composed(&self) -> Vec<String> {
        let mut out = self.head.clone();
        if self.total > WINDOW_LIMIT as u64 {
            out.extend(self.tail.iter().cloned());
            return out;
        }
        // Small totals: tail holds rows [total-50, total); skip the rows
        // already emitted by head so every scrolled row appears once.
        let head_len = self.head.len();
        let skip = if self.total <= TAIL_LINES as u64 {
            head_len
        } else {
            (WINDOW_LIMIT as u64 - self.total) as usize
        };
        out.extend(self.tail.iter().skip(skip).cloned());
        out
    }
}
