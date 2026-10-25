//! Streaming regex search over files (ripgrep-family crates), honoring the
//! same gitignore rules as the index.
//!
//! Results stream lazily: [`grep_stream`] returns an [`Iterator`] that walks
//! files on demand — callers can `take(n)` without paying for the rest of
//! the corpus. Hit windows are bounded ([`GrepOptions::max_snippet_bytes`])
//! and nearby matches merge into one entry (multi-line dedupe). Binary
//! files (NUL sniff) are quit-early skipped; oversize files are skipped
//! outright; both paths never panic and never pollute results.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkFinish, SinkMatch};

use crate::error::SearchError;
use crate::options::GrepOptions;

/// One grep result window.
#[derive(Debug, Clone)]
pub struct GrepHit {
    /// Root-relative path with `/` separators.
    pub path: String,
    /// First matched line, 1-based.
    pub line_start: u64,
    /// Last matched line covered by this window (>= `line_start`).
    pub line_end: u64,
    /// Bounded lossy-UTF-8 snippet of the matched lines.
    pub snippet: String,
}

impl GrepHit {
    fn push_line(&mut self, line: &str, max_bytes: usize) {
        if self.snippet.len() >= max_bytes {
            return;
        }
        if !self.snippet.is_empty() {
            self.snippet.push('\n');
        }
        let budget = max_bytes - self.snippet.len();
        self.snippet.push_str(truncate_utf8(line, budget));
    }
}

fn truncate_utf8(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn compile_matcher(pattern: &str, options: &GrepOptions) -> Result<RegexMatcher, SearchError> {
    grep_regex::RegexMatcherBuilder::new()
        .case_insensitive(options.case_insensitive)
        .fixed_strings(options.literal)
        .build(pattern)
        .map_err(|err| SearchError::InvalidRegex(err.to_string()))
}

/// Compiles `pattern` (typed [`SearchError::InvalidRegex`] on bad syntax)
/// and returns a lazy hit iterator over `root`.
///
/// Pattern mode and result caps come from `options`; see [`GrepOptions`].
/// Per-file I/O failures skip that file and keep the stream alive.
pub fn grep_stream(
    root: &Path,
    pattern: &str,
    options: &GrepOptions,
) -> Result<impl Iterator<Item = GrepHit>, SearchError> {
    let matcher = compile_matcher(pattern, options)?;
    let root = root.canonicalize()?;
    let walker = ignore::WalkBuilder::new(&root)
        .standard_filters(true)
        .build();
    let searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(0x00))
        .build();
    Ok(GrepIter {
        root,
        walker,
        matcher,
        searcher,
        options: options.clone(),
        pending: VecDeque::new(),
        emitted: 0,
        exhausted: false,
    })
}

struct GrepIter {
    root: PathBuf,
    walker: ignore::Walk,
    matcher: RegexMatcher,
    searcher: Searcher,
    options: GrepOptions,
    pending: VecDeque<GrepHit>,
    emitted: usize,
    exhausted: bool,
}

impl Iterator for GrepIter {
    type Item = GrepHit;

    fn next(&mut self) -> Option<GrepHit> {
        loop {
            if let Some(hit) = self.pending.pop_front() {
                self.emitted += 1;
                return Some(hit);
            }
            if self.exhausted || self.emitted >= self.options.max_hits {
                return None;
            }
            if !self.advance_one_file() {
                continue;
            }
        }
    }
}

impl GrepIter {
    /// Searches walker files until at least one hit is pending or the walk
    /// is exhausted. Returns `true` when `pending` is non-empty.
    fn advance_one_file(&mut self) -> bool {
        for entry in self.walker.by_ref() {
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if !metadata.is_file() || metadata.len() > self.options.max_file_bytes {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(&self.root) else {
                continue;
            };
            let sink = HitSink {
                rel_path: rel.to_string_lossy().replace('\\', "/"),
                options: &self.options,
                cap: self.options.max_hits,
                current: None,
                out: &mut self.pending,
            };
            let _ = self.searcher.search_path(&self.matcher, entry.path(), sink);
            if self.pending.len() >= self.options.max_hits {
                self.exhausted = true;
            }
            if !self.pending.is_empty() {
                return true;
            }
        }
        self.exhausted = true;
        false
    }
}

struct HitSink<'a> {
    rel_path: String,
    options: &'a GrepOptions,
    cap: usize,
    current: Option<GrepHit>,
    out: &'a mut VecDeque<GrepHit>,
}

impl HitSink<'_> {
    fn flush(&mut self) {
        if let Some(hit) = self.current.take() {
            self.out.push_back(hit);
        }
    }
}

impl Sink for HitSink<'_> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        matched: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        let line_number = matched.line_number().unwrap_or(1);
        let text = String::from_utf8_lossy(matched.bytes());
        let text = text.trim_end_matches('\n').trim_end_matches('\r');
        let line_count = text.matches('\n').count() as u64 + 1;
        let end_line = line_number + line_count - 1;

        match &mut self.current {
            Some(current) if line_number <= current.line_end + self.options.merge_gap_lines + 1 => {
                current.line_end = current.line_end.max(end_line);
                current.push_line(text, self.options.max_snippet_bytes);
            }
            _ => {
                self.flush();
                let mut hit = GrepHit {
                    path: self.rel_path.clone(),
                    line_start: line_number,
                    line_end: end_line,
                    snippet: String::new(),
                };
                hit.push_line(text, self.options.max_snippet_bytes);
                self.current = Some(hit);
            }
        }

        let filled = self.out.len() + usize::from(self.current.is_some());
        Ok(filled < self.cap)
    }

    fn finish(&mut self, _searcher: &Searcher, _: &SinkFinish) -> Result<(), std::io::Error> {
        self.flush();
        Ok(())
    }
}
