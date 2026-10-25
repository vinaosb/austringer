//! CRLF-correct line table: the shared line-math foundation for every tool
//! in this crate (windows repo — files may be CRLF, LF, or mixed).
//!
//! Invariants:
//! - Line numbers are **1-based** everywhere in the public API.
//! - `content(n)` never contains a terminator; `ending(n)` says which the
//!   original file used (`Lf`, `Crlf`, or `Eof` for a final unterminated
//!   line), so edits can preserve each line's own ending byte-for-byte.
//! - Construction rejects non-UTF-8 input with a typed error (these tools
//!   are line/text oriented; binary content is out of contract).
//! - A `\r` that is not part of a `\r\n` pair is ordinary content, never a
//!   terminator (mixed-CRLF/LF files round-trip exactly).

use crate::error::ToolsError;
use std::path::PathBuf;

/// The terminator kind that followed a line in the original bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// `\n`.
    Lf,
    /// `\r\n` (Windows default).
    Crlf,
    /// Final line with no trailing terminator.
    Eof,
}

/// One line's layout inside [`LineTable`].
#[derive(Debug, Clone, Copy)]
struct LineEntry {
    /// Byte offset of the first content byte.
    start: usize,
    /// Content length in bytes, terminator excluded.
    len: usize,
    /// Which terminator followed this line.
    ending: LineEnding,
}

/// A UTF-8 source split into CRLF-aware lines with byte-exact round-trip.
#[derive(Debug, Clone)]
pub struct LineTable {
    text: String,
    entries: Vec<LineEntry>,
}

impl LineTable {
    /// Split UTF-8 bytes into lines. Errors: [`ToolsError::NonUtf8`].
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, ToolsError> {
        let text = String::from_utf8(bytes).map_err(|_| ToolsError::NonUtf8 {
            path: PathBuf::new(),
        })?;
        Ok(Self::from_string(text))
    }

    /// Split an owned string (no UTF-8 check needed).
    pub fn from_string(text: String) -> Self {
        let mut entries = Vec::new();
        let bytes = text.as_bytes();
        let mut start = 0;
        let mut index = 0;
        while index < bytes.len() {
            match bytes[index] {
                b'\n' => {
                    entries.push(LineEntry {
                        start,
                        len: index - start,
                        ending: LineEnding::Lf,
                    });
                    index += 1;
                    start = index;
                }
                b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                    entries.push(LineEntry {
                        start,
                        len: index - start,
                        ending: LineEnding::Crlf,
                    });
                    index += 2;
                    start = index;
                }
                _ => index += 1,
            }
        }
        if start < bytes.len() {
            entries.push(LineEntry {
                start,
                len: bytes.len() - start,
                ending: LineEnding::Eof,
            });
        }
        LineTable { text, entries }
    }

    /// Number of lines (an empty file has 0).
    pub fn line_count(&self) -> usize {
        self.entries.len()
    }

    /// Content of a 1-based line, terminator excluded. `None` when out of
    /// range.
    pub fn content_opt(&self, line: usize) -> Option<&str> {
        let entry = self.entries.get(line.checked_sub(1)?)?;
        self.text.get(entry.start..entry.start + entry.len)
    }

    /// Terminator kind of a 1-based line. `None` when out of range.
    pub fn ending(&self, line: usize) -> Option<LineEnding> {
        self.entries.get(line.checked_sub(1)?).map(|e| e.ending)
    }

    /// 1-based line containing byte `offset` (clamped into the file; an
    /// offset at EOF maps to the last line).
    pub fn line_of_byte(&self, offset: usize) -> usize {
        let clamp = offset.min(self.text.len());
        match self.entries.binary_search_by(|e| e.start.cmp(&clamp)) {
            Ok(index) => index + 1,
            Err(0) => 1,
            Err(next) => next,
        }
    }

    /// Iterator over `(1-based line, content)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &str)> {
        self.entries.iter().enumerate().map(|(index, entry)| {
            (
                index + 1,
                self.text
                    .get(entry.start..entry.start + entry.len)
                    .unwrap_or_default(),
            )
        })
    }

    /// All line contents (terminator-free), in order.
    pub fn contents(&self) -> Vec<&str> {
        self.iter().map(|(_, content)| content).collect()
    }

    /// The terminator used by the majority of lines (ties → `Crlf`, the
    /// Windows-safe default; empty files → `Lf`).
    pub fn dominant_ending(&self) -> LineEnding {
        let crlf = self
            .entries
            .iter()
            .filter(|e| e.ending == LineEnding::Crlf)
            .count();
        let lf = self
            .entries
            .iter()
            .filter(|e| e.ending == LineEnding::Lf)
            .count();
        if crlf >= lf && crlf > 0 {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        }
    }

    /// Full original bytes (exact round-trip).
    pub fn as_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// Full original text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Consumes the table into the original text.
    pub fn into_string(self) -> String {
        self.text
    }
}

/// Terminator as the exact bytes written after a line.
pub fn line_ending_bytes(ending: LineEnding) -> &'static str {
    match ending {
        LineEnding::Lf => "\n",
        LineEnding::Crlf => "\r\n",
        LineEnding::Eof => "",
    }
}

/// Split `text` into logical lines for content comparison: splits on `\n`
/// and drops a single trailing `\r` from each part. A trailing newline
/// yields no trailing empty part (`"a\n"` → `["a"]`, matching how the
/// table reports files ending in a newline). Used by edit anchors so a
/// model's LF-formatted `old_text` matches a CRLF file.
pub fn logical_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<&str> = text.split('\n').collect();
    if parts.last() == Some(&"") {
        parts.pop();
    }
    parts
        .into_iter()
        .map(|part| part.strip_suffix('\r').unwrap_or(part))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_cr_is_content_not_terminator() {
        let table = LineTable::from_string("a\rb\nc".to_owned());
        assert_eq!(table.line_count(), 2);
        assert_eq!(table.content_opt(1), Some("a\rb"));
    }

    #[test]
    fn bare_newline_file_is_one_empty_line() {
        let table = LineTable::from_string("\n".to_owned());
        assert_eq!(table.line_count(), 1);
        assert_eq!(table.content_opt(1), Some(""));
        assert_eq!(table.ending(1), Some(LineEnding::Lf));
    }

    #[test]
    fn logical_lines_normalizes_crlf_and_trailing_newline() {
        assert_eq!(logical_lines("a\r\nb\r\n"), vec!["a", "b"]);
        assert_eq!(logical_lines("a\nb"), vec!["a", "b"]);
        assert!(logical_lines("").is_empty());
    }
}
