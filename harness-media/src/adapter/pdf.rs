//! # PDF adapter (plan M13: pure tier - extract + minimal write)
//!
//! Digest section 3.1 verdict: PDF is GREEN (ISO 32000-2:2020 free via
//! pdfa.org; Adobe Public Patent License, RF, defensive suspension only).
//! MIME `application/pdf`; magic `%PDF-` (M2 probe lane).
//!
//! ## Dependency decisions (executor's pick, cargo-deny verified)
//!
//! - **Extract: lopdf 0.42** over pdf-extract. Both are pure Rust
//!   (MIT; no C, MSVC-clean), but the needed surface decides: this
//!   adapter must surface page count, version, Info metadata, outline
//!   depth, a FONT LIST WITH EMBEDDED-NESS, embedded-image count, the
//!   per-page text layer, and encryption detection. lopdf exposes the
//!   document DOM for all of that AND a text decoder that handles both
//!   simple-font encoding tables (WinAnsi/Standard) and Type0 fonts
//!   through their ToUnicode CMaps - the exact fidelity surface the
//!   round-trip gate needs. pdf-extract is text-only (no font list, no
//!   metadata, no outline lane) and would drag a second lopdf copy.
//!   Cargo-deny verdict recorded: the digest named lopdf 0.40.0, but
//!   RUSTSEC-2026-0187 (stack overflow in `load_mem` via deeply nested
//!   objects - this adapter's exact untrusted-parse surface, fixed in
//!   0.42.0) forces the pin to >=0.42.
//! - **Generate: krilla 0.8.2** (plan first choice; digest-verified).
//!   High-level modern writer on pdf-writer: subsets and EMBEDS every
//!   used font and writes ToUnicode CMaps, which is precisely the #1
//!   agent-PDF defect this adapter must never commit (unembedded fonts
//!   / WinAnsi-for-non-Latin1). Byte-deterministic: its document and
//!   instance IDs are content-hash derived, never random. Cargo-deny
//!   verdict recorded: krilla's text shaping rides rustybuzz/ttf-parser
//!   (RUSTSEC-2026-0206/-0192, UNMAINTAINED-not-vulnerable, no 0.8.x
//!   upgrade path; ignored in deny.toml with rationale) - shaping runs
//!   only on validated spec text, never on untrusted input.
//!
//! ## Fidelity laws (pinned by tests/pdf_roundtrip.rs)
//!
//! - Never emit WinAnsi for non-Latin1 text: krilla writes Type0
//!   (Identity-H) fonts with ToUnicode maps for everything, and the
//!   round-trip corpus asserts the extract font list shows Type0 +
//!   embedded for the Cyrillic/CJK entries.
//! - Producer disclosure is honest about scope: the generate engine id
//!   ([`GENERATE_ENGINE`]) states SIMPLE TEXT LAYOUT - no
//!   justification, no hyphenation, no pagination claims.
//!
//! ## Honesty boundaries
//!
//! - Embedded JS / launch actions are LISTED, never run - no evaluator
//!   exists in this adapter, and none is added.
//! - Encrypted documents fail the typed
//!   [`MediaError::EncryptedUnsupported`] before any stream is read.
//! - Budget enforcement covers this adapter's extraction loop (deadline
//!   checked per page; cumulative materialization counted against
//!   `max_total_bytes`, tripping typed [`MediaError::ExtractTooLarge`]).
//!   The initial document parse runs inside lopdf; a hostile
//!   pathological input is bounded in practice by the materialization
//!   accounting, not by interrupting lopdf's parser (recorded in the
//!   M13 evidence).
//! - Per-page text is bounded by the caller's budget: overflow is the
//!   typed ExtractTooLarge refusal (honest), never silent truncation.

use harness_core::event::ProjectionKind;
use serde_json::{json, Value};

use crate::error::MediaError;
use crate::generate::GenerationSpec;
use crate::projection::{ProjectionContext, ProjectionProvider};

pub use super::pdf_extract::extract;
pub use super::pdf_generate::generate;
pub use super::pdf_verify::verify;

/// Producer engine id for the extract/verify bindings.
pub const EXTRACT_ENGINE: &str = "harness-media::pdf";

/// Producer engine id for the generate binding. Carries the plan-mandated
/// SIMPLE TEXT LAYOUT disclosure (no justification/hyphenation claims).
pub const GENERATE_ENGINE: &str = "harness-media::pdf-krilla-simple-layout";

/// Producer engine id for the verify binding (text diff gate).
pub const VERIFY_ENGINE: &str = "harness-media::pdf";

/// Collapses every whitespace run (spaces, newlines, tabs) to a single
/// space and trims the ends - the normalization the round-trip text diff
/// runs under (plan M13: "text diff with whitespace normalization").
#[must_use]
pub fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The INDEPENDENT spec->tree projection (M5 provenance rule: never the
/// adapter's own extractor over its own output). Projects the generation
/// spec to the same comparable form [`verify`] extracts: page count plus
/// whitespace-normalized per-page text.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec does not satisfy the pdf
/// adapter contract (same parser the writer enforces).
pub fn spec_tree(spec: &GenerationSpec) -> Result<Value, MediaError> {
    let parsed = super::pdf_generate::parse_spec(spec)?;
    let pages: Vec<Value> = parsed
        .pages
        .iter()
        .map(|page| {
            let joined: Vec<&str> = page.blocks.iter().map(|block| block.text.as_str()).collect();
            json!({"text": normalize_ws(&joined.join("\n"))})
        })
        .collect();
    Ok(json!({"page_count": pages.len(), "pages": pages}))
}

/// Deterministic pdf text projector (the M10 seam: a thin wrap over
/// [`crate::extract::Extracted::text_projection`], registered alongside
/// this adapter).
pub struct PdfProjection;

impl ProjectionProvider for PdfProjection {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}
