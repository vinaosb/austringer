//! Strict mode (plan todo 16, clause 7 — Mentat day-1 invariant): write
//! authority scoped to read scope. Edits touching lines that were not
//! previously read in the session are a typed rejection.
//!
//! [`ReadLedger`] is the trait (the daemon may back it with the session
//! store later); [`MemoryLedger`] is the in-memory implementation. The
//! ledger records 1-based inclusive read windows (from `view_window`,
//! block reads, ...) and answers coverage queries with merged windows.

use crate::error::ToolsError;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

/// Tracks which line windows of which files were read this session.
pub trait ReadLedger {
    /// Record that `[first, last]` (1-based, inclusive) of `path` entered
    /// the model's context.
    fn record_read(&mut self, path: &Path, first: usize, last: usize);

    /// Merged, sorted, non-overlapping read windows of `path` as
    /// `(first, last)` inclusive pairs.
    fn read_windows(&self, path: &Path) -> Vec<(usize, usize)>;

    /// Whether `[first, last]` lies entirely inside recorded windows.
    fn covers(&self, path: &Path, lines: &RangeInclusive<usize>) -> bool {
        let first = *lines.start();
        let last = *lines.end();
        self.read_windows(path)
            .iter()
            .any(|(lo, hi)| first >= *lo && last <= *hi)
    }
}

/// In-memory [`ReadLedger`].
#[derive(Debug, Default, Clone)]
pub struct MemoryLedger {
    entries: HashMap<PathBuf, Vec<(usize, usize)>>,
}

impl MemoryLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ReadLedger for MemoryLedger {
    fn record_read(&mut self, path: &Path, first: usize, last: usize) {
        if first > last {
            return;
        }
        let windows = self.entries.entry(path.to_path_buf()).or_default();
        windows.push((first, last));
        windows.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(windows.len());
        for &(lo, hi) in windows.iter() {
            match merged.last_mut() {
                Some((_, last_hi)) if lo <= last_hi.saturating_add(1) => {
                    if hi > *last_hi {
                        *last_hi = hi;
                    }
                }
                _ => merged.push((lo, hi)),
            }
        }
        *windows = merged;
    }

    fn read_windows(&self, path: &Path) -> Vec<(usize, usize)> {
        self.entries.get(path).cloned().unwrap_or_default()
    }
}

/// Strict-mode gate: every line range an edit will touch must already be
/// covered by the ledger. Returns the list of missing sub-ranges as a
/// typed [`ToolsError::UnreadRegion`].
pub fn enforce_read_scope(
    ledger: &dyn ReadLedger,
    path: &Path,
    edit_ranges: &[RangeInclusive<usize>],
) -> Result<(), ToolsError> {
    let windows = ledger.read_windows(path);
    let mut missing: Vec<RangeInclusive<usize>> = Vec::new();
    for range in edit_ranges {
        let mut line = *range.start();
        let end = *range.end();
        'range_walk: while line <= end {
            for &(lo, hi) in &windows {
                if line >= lo && line <= hi {
                    line = hi + 1;
                    continue 'range_walk;
                }
            }
            let gap_start = line;
            while line <= end {
                let inside = windows.iter().any(|&(lo, hi)| line >= lo && line <= hi);
                if inside {
                    break;
                }
                line += 1;
            }
            missing.push(gap_start..=(line - 1));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ToolsError::UnreadRegion {
            path: path.to_path_buf(),
            missing,
        })
    }
}
