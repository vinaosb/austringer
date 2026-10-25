//! Foreign-session import (Grok foreign-sessions): best-effort readers
//! mapping Claude Code and Codex session files onto T4 events, with a
//! typed [`UnknownForeignEntry`] for everything that cannot map
//! losslessly.

pub mod claude;
pub mod codex;

use austringer_core::event::Event;

use crate::import::ImportOrigin;

/// A foreign line that could not be mapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownForeignEntry {
    /// 1-based line number in the source file.
    pub line: u64,
    /// The foreign `type` tag when the line parsed (e.g. `summary`),
    /// else `malformed`.
    pub kind: String,
    /// Why it was not mapped.
    pub reason: ForeignReason,
}

/// Why a foreign entry stayed unmapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignReason {
    /// A recognized type with no T4 counterpart (disclosed lossy).
    UnmappedType,
    /// A payload shape T4 cannot represent (e.g. Claude inline images,
    /// which violate the out-of-line media rule).
    UnsupportedShape {
        /// What the shape was.
        detail: String,
    },
    /// The line is not valid JSON.
    MalformedJson,
    /// Valid JSON of the wrong shape (missing required fields).
    WrongShape {
        /// What was missing or malformed.
        detail: String,
    },
}

/// The result of reading a foreign session file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ForeignRead {
    /// Events mapped onto the T4 taxonomy, file order.
    pub events: Vec<Event>,
    /// Everything that could not be mapped, file order.
    pub unknown: Vec<UnknownForeignEntry>,
    /// Whether an unterminated tail line was skipped.
    pub torn_tail: bool,
}

impl ForeignRead {
    /// Import origin for this read.
    #[must_use]
    pub fn origin(&self, source: &str) -> ImportOrigin {
        ImportOrigin::foreign(source, self.unknown.len() as u64)
    }
}

pub(crate) fn read_lines(raw: &[u8], mut on_line: impl FnMut(u64, &[u8])) -> bool {
    let ends_with_newline = raw.last() == Some(&b'\n');
    let segments: Vec<&[u8]> = raw.split(|&b| b == b'\n').collect();
    let mut torn_tail = false;
    for (index, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }
        let line = segment.strip_suffix(b"\r").unwrap_or(segment);
        let is_last = index == segments.len() - 1;
        if is_last
            && !ends_with_newline
            && serde_json::from_slice::<serde_json::Value>(line).is_err()
        {
            torn_tail = true;
            continue;
        }
        on_line(index as u64 + 1, line);
    }
    torn_tail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_lines_flags_torn_tail_and_numbers_lines() {
        let mut seen = Vec::new();
        let torn = read_lines(b"{\"a\":1}\n{\"b\":2}\n{\"torn", |line_no, line| {
            seen.push((line_no, String::from_utf8_lossy(line).to_string()));
        });
        assert!(torn);
        assert_eq!(
            seen,
            vec![(1, "{\"a\":1}".to_string()), (2, "{\"b\":2}".to_string())]
        );
    }

    #[test]
    fn complete_final_line_without_newline_is_kept() {
        let mut seen = Vec::new();
        let torn = read_lines(b"{\"a\":1}\n{\"b\":2}", |line_no, line| {
            seen.push((line_no, line.len()));
        });
        assert!(!torn);
        assert_eq!(seen, vec![(1, 7), (2, 7)]);
    }
}
