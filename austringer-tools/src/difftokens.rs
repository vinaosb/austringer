//! The imara-diff integration seam: logical (terminator-free) line
//! tokenization + a Histogram diff that yields hunks over 0-based line
//! indexes. Comparing logical lines makes the diff CRLF-insensitive — a
//! CRLF re-encode of an unchanged region never shows as changed.

use imara_diff::TokenSource;
use imara_diff::{Algorithm, Diff, InternedInput};

/// A token source over pre-extracted logical line contents.
pub(crate) struct LogicalLines<'a>(pub &'a [&'a str]);

impl<'a> TokenSource for LogicalLines<'a> {
    type Token = &'a str;
    type Tokenizer = std::iter::Copied<std::slice::Iter<'a, &'a str>>;
    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter().copied()
    }
    fn estimate_tokens(&self) -> u32 {
        self.0.len() as u32
    }
}

/// Postprocessed Histogram hunks between two logical line sequences.
pub(crate) fn histogram_hunks(old: &[&str], new: &[&str]) -> Vec<imara_diff::Hunk> {
    let input = InternedInput::new(LogicalLines(old), LogicalLines(new));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    diff.hunks().collect()
}
