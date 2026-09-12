//! Typed errors for every failure mode the editing tools can produce.
//! Each variant is a distinct tool-contract outcome (never a stringly error),
//! so the agent loop can feed structured feedback back to the model.

use std::ops::RangeInclusive;
use std::path::PathBuf;
use thiserror::Error;

/// All fallible `harness-tools` operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ToolsError {
    /// Underlying filesystem failure (missing file, locked file, over-long
    /// path rejected by the OS, ...).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// A file (or edit payload) is not valid UTF-8. The tools are line/text
    /// oriented; binary content is a typed rejection, never lossy rendering.
    #[error("content of {path} is not valid UTF-8")]
    NonUtf8 {
        /// Path as reported by the caller (`-` for in-memory content).
        path: PathBuf,
    },

    /// A 1-based line reference lies outside the file.
    #[error("line {line} is out of range (file has {total} lines)")]
    LineOutOfRange {
        /// The requested 1-based line.
        line: usize,
        /// Total lines in the file.
        total: usize,
    },

    /// An edit list carries no edits.
    #[error("edit list is empty")]
    NoEdits,

    /// `old_text` of an edit is empty (insertions are expressed as
    /// old_text = the anchor line's content; see `edit` docs).
    #[error("edit at line {line} has empty old_text; anchor on the surrounding line instead")]
    EmptyOldText {
        /// The edit's 1-based anchor line.
        line: usize,
    },

    /// A single edit's payload exceeds the structural write cap that keeps
    /// every edit a bounded patch — whole-file rewrites have no API here.
    #[error("edit produces {new_lines} new lines, over the {max}-line per-edit cap")]
    EditTooLarge {
        /// New-line count of the offending edit.
        new_lines: usize,
        /// The cap ([`crate::edit::MAX_NEW_LINES_PER_EDIT`]).
        max: usize,
    },

    /// More edits in one call than the batch cap.
    #[error("{count} edits exceed the {max}-edit per-call cap")]
    TooManyEdits {
        /// Actual edit count.
        count: usize,
        /// The cap ([`crate::edit::MAX_EDITS_PER_CALL`]).
        max: usize,
    },

    /// `old_text` does not match the anchored lines (external modification,
    /// wrong line number, or stale view). Never silently jumps to a
    /// first-match elsewhere.
    #[error("old_text does not match line {line}: expected {expected:?}, found {actual:?}")]
    AnchorMismatch {
        /// The 1-based line that was anchored.
        line: usize,
        /// Expected first logical line (truncated in display).
        expected: String,
        /// Actual first logical line (truncated in display).
        actual: String,
    },

    /// `old_text` occurs more than once in the file — the anchor is
    /// ambiguous and the edit is rejected (never first-match applied).
    #[error("old_text at line {line} is ambiguous: also matches at {occurrences:?}")]
    AmbiguousAnchor {
        /// The anchored 1-based line.
        line: usize,
        /// All 1-based lines where the old_text sequence starts.
        occurrences: Vec<usize>,
    },

    /// A hashline anchor failed to re-anchor after drift: no candidate (or
    /// several equally-plausible candidates) within the re-anchor radius.
    #[error(
        "hashline anchor for line {anchor_line} drifted and could not be re-anchored (candidates: {candidates:?})"
    )]
    AnchorDrift {
        /// The anchor's recorded 1-based line.
        anchor_line: usize,
        /// Candidate lines found by the re-anchor scan (empty = none).
        candidates: Vec<usize>,
    },

    /// Two edits in one batch touch overlapping line ranges.
    #[error("edits #{first} and #{second} overlap")]
    OverlappingEdits {
        /// Index of the first overlapping edit.
        first: usize,
        /// Index of the second overlapping edit.
        second: usize,
    },

    /// Strict mode: the edit touches lines that were never read in this
    /// session (write authority is scoped to read scope).
    #[error("strict mode: {path} lines {missing:?} were never read in this session")]
    UnreadRegion {
        /// The file being edited.
        path: PathBuf,
        /// Unread 1-based line ranges (inclusive) the edit tried to touch.
        missing: Vec<RangeInclusive<usize>>,
    },

    /// The post-edit syntax gate rejected the edit (the AST parse found new
    /// errors the file did not have before the edit).
    #[error("edit rejected by syntax gate: {count} new diagnostic(s): {first}")]
    SyntaxGate {
        /// Number of new diagnostics.
        count: usize,
        /// First new diagnostic, rendered `line:col message`.
        first: String,
        /// Structured new diagnostics (capped).
        diagnostics: Vec<harness_ast::Diagnostic>,
    },

    /// The syntax gate's parse engine failed for a non-degrading reason
    /// (e.g. over the bounded-source cap); the edit is refused rather
    /// than applied ungated.
    #[error("syntax gate engine failure: {0}")]
    GateEngine(#[from] harness_ast::AstError),

    /// Decision vector shape does not match the staged group it applies to
    /// (an `Expand` decision must carry one decision per replacement).
    #[error("decision for group {group} has {got} replacement decisions, expected {expected}")]
    InvalidDecisions {
        /// Zero-based group index.
        group: usize,
        /// Replacements in the group.
        expected: usize,
        /// Decisions supplied.
        got: usize,
    },

    /// A block-read anchor pointed at a blank line (no indentation
    /// structure to expand from).
    #[error("anchor line {line} is blank; block reads need a non-blank anchor")]
    BlankAnchor {
        /// The requested 1-based anchor line.
        line: usize,
    },
}
