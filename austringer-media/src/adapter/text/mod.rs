//! # Text-substrate adapters (plan M30) - the five thin honest lanes
//!
//! Markdown (CommonMark 0.31.2 + GFM via comrak), HTML5 (maud
//! generation + html5gum reparse), JSON/JSONL (RFC 8259 serde_json
//! canonical), CSV (RFC 4180 via the csv crate), TOML (1.0 via the toml
//! crate). All pure tier, all GREEN rows in the digest licensing table.
//! Verification is the M5 canonical reparse -> structural diff for every
//! lane. YAML is deliberately ABSENT (digest 6 DEFERRED; prefer TOML
//! emit; serde_yml is banned outright - standing constraint 7).
//!
//! ## Honesty rows (surfaced verbatim by the M27 matrix)
//!
//! - **Markdown**: dialect pinned to CommonMark 0.31.2 + GFM (comrak's
//!   exact compliance target); the writer renders through comrak's
//!   canonical CommonMark formatter, so generated output is one
//!   canonical spelling (ATX headings, `-` bullets) - the dialect-bleed
//!   risk (setext vs ATX, `*` vs `_`) is pinned by parsing both
//!   spellings to the SAME census tree, never by byte comparison.
//!   Inline formatting (emphasis/links) is NOT in the generation subset;
//!   block text is single-line and whitespace-collapsed (typed
//!   `InvalidSpec` otherwise - never a silent rewrite).
//! - **HTML5**: WHATWG Living Standard, `text/html`; LOSSY as
//!   interchange (no pagination, and scripts/JS are never executed -
//!   standing rule). Generation emits a SELF-CONTAINED single file:
//!   `<!DOCTYPE html>` + `<meta charset="utf-8">`, no external assets
//!   (no src/href/link/script), maud-escaped text. Extraction is a
//!   TOKENIZER census (html5gum), not a DOM build: HTML5 parsing has no
//!   error concept (error recovery is the spec), so a doctype-less
//!   fragment is a census fact, never a failure. UTF-8 only -
//!   `<meta charset>` sniffing for other encodings is honestly out of
//!   scope. Tag census counts START tags (void/self-closing included);
//!   end tags mirror start tags except for void elements.
//! - **JSON/JSONL**: RFC 8259 / I-JSON via serde_json with the
//!   workspace's sorted-key stability (the `preserve_order` feature is
//!   banned): reads canonicalize to sorted-key compact form; duplicate
//!   keys take the last value (serde_json; the RFC permits), and the
//!   canonical form is what round trips. The JSONL lane (one value per
//!   non-empty line) is auto-detected exactly like the M2 probe: full
//!   single-value parse first, line-parse second, both failing is the
//!   typed `ParseFailure`. serde_json's recursion cap bounds nesting.
//! - **CSV**: RFC 4180, comma dialect ONLY - the dialect chaos the
//!   digest names is never guessed: semicolon/tab files parse as
//!   single-field comma-dialect rows, ragged rows refuse typed, and the
//!   writer never emits a BOM (write-BOM disclosed: we do not write
//!   one). Reading is BOM-TOLERANT: a UTF-8 BOM is stripped and
//!   disclosed as `"bom": true` in the tree. Rows are counted
//!   uniformly - a header row is a semantic convention CSV cannot
//!   express, so `first_row` is reported, never guessed at. The writer
//!   is CRLF-terminated per RFC 4180 and quotes per the RFC.
//! - **TOML**: 1.0 via the toml crate. Extract normalizes the parse
//!   tree to JSON (datetimes as their TOML display strings; non-finite
//!   floats as `null` per the M5 pinned decision). Generation accepts
//!   JSON-shaped tables and refuses what TOML cannot represent - root
//!   non-table, `null`, heterogeneous arrays - typed `InvalidSpec`,
//!   never a mangled write.
//!
//! ## Budgets (M1 extract safety)
//!
//! Loop lanes (markdown AST walk, html tokenization, csv records,
//! jsonl lines) check the wall-clock deadline and cumulative-size cap
//! every iteration. Single-shot parsers (serde_json, toml, comrak's
//! parse) are bounded by an input-size pre-check against
//! `max_total_bytes` plus a deadline check after; their internal
//! recursion/nesting caps bound depth. Every trip is the typed
//! [`crate::error::MediaError::ExtractTooLarge`] - never a hang.

pub mod csv;
pub mod html;
pub mod json;
pub mod markdown;
pub mod toml;

/// Whitespace-collapse: runs of whitespace become one space, ends
/// trimmed (the canonical census text form shared by every lane).
pub(crate) fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate to at most `max` CHARS, at a char boundary.
pub(crate) fn truncate_chars(raw: String, max: usize) -> String {
    match raw.char_indices().nth(max) {
        Some((index, _)) => raw[..index].to_owned(),
        None => raw,
    }
}

/// The generation-substrate text contract: single line, no leading/
/// trailing whitespace, no internal whitespace runs, non-empty when
/// `require_nonempty`.
pub(crate) fn is_canonical_text(text: &str, require_nonempty: bool) -> bool {
    if text.contains(['\n', '\r']) || text != collapse(text) {
        return false;
    }
    !require_nonempty || !text.is_empty()
}

/// A single tolerance profile shared by every text lane: float epsilon
/// only. No unordered pointers (spec-driven text writes are
/// ordered-exact), no ignored subtrees.
pub(crate) fn tolerances() -> crate::diff::Tolerances {
    crate::diff::Tolerances {
        float_epsilon: 1e-9,
        unordered_at: std::collections::BTreeSet::new(),
        ignored_at: std::collections::BTreeSet::new(),
    }
}
