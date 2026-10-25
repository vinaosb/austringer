//! Tunable caps for indexing and grep. Defaults are chosen for a code-search
//! workload: small-to-medium text files, bounded result windows, and bounded
//! memory. Every field is documented with its unit.

/// Knobs for [`crate::CodeIndex::refresh_with`].
#[derive(Debug, Clone)]
pub struct IndexOptions {
    /// Files larger than this many bytes are skipped (`skipped_oversize`).
    pub max_file_bytes: u64,
    /// Binary sniff window: a NUL within this many leading bytes marks the
    /// file binary and it is never indexed.
    pub sniff_bytes: usize,
    /// Bytes of the file head stored verbatim for snippet generation.
    pub head_bytes: usize,
    /// Tantivy writer arena size in bytes (per refresh, single writer thread).
    pub writer_heap: usize,
}

impl IndexOptions {
    /// Defaults: 16 MiB file cap, 8 KiB sniff/head windows, 32 MiB arena.
    pub const fn default() -> Self {
        Self {
            max_file_bytes: 16 * 1024 * 1024,
            sniff_bytes: 8 * 1024,
            head_bytes: 8 * 1024,
            writer_heap: 32 * 1024 * 1024,
        }
    }
}

/// Knobs for [`crate::grep_stream`].
///
/// The pattern-mode flags (`literal`, `case_insensitive`) are applied when
/// the regex is compiled; the remaining fields bound result size and memory.
#[derive(Debug, Clone)]
pub struct GrepOptions {
    /// Compile the pattern as a fixed literal string (ripgrep `-F`).
    pub literal: bool,
    /// Compile the pattern case-insensitively (ripgrep `-i`).
    pub case_insensitive: bool,
    /// Files larger than this many bytes are skipped entirely.
    pub max_file_bytes: u64,
    /// Maximum number of hits returned across the whole stream.
    pub max_hits: usize,
    /// Snippet byte cap per hit.
    pub max_snippet_bytes: usize,
    /// Matches whose line numbers differ by at most this many lines merge
    /// into one windowed entry (multi-line dedupe).
    pub merge_gap_lines: u64,
}

impl GrepOptions {
    /// Defaults: regex mode, case-sensitive, 16 MiB file cap, 200 hits,
    /// 512-byte snippets, merge gap of 1 line.
    pub const fn default() -> Self {
        Self {
            literal: false,
            case_insensitive: false,
            max_file_bytes: 16 * 1024 * 1024,
            max_hits: 200,
            max_snippet_bytes: 512,
            merge_gap_lines: 1,
        }
    }
}
