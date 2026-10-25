//! # Markdown adapter, pure tier (plan M30)
//!
//! comrak (CommonMark 0.31.2 + GFM) drives both sides: extract walks the
//! AST into the block census, generate builds the AST from the spec and
//! renders through comrak's canonical CommonMark formatter, verify
//! reparses and diffs (M5). The dialect is pinned: generated documents
//! are always the canonical spelling (ATX headings, `-`/`1.` markers,
//! backtick fences), and the dialect-bleed risk is disclosed by the
//! setext-vs-ATX census equivalence - never a byte comparison.
//!
//! ## Honesty rows
//!
//! - Licensing (digest 3.1): CommonMark 0.31.2 + GFM, MIME
//!   `text/markdown;charset=utf-8` (RFC 7763), GREEN (CC-BY-SA spec
//!   docs, zero patents).
//! - CommonMark is TOTAL over utf8 text: there is no malformed markdown,
//!   only bytes that are not utf8 (typed `ParseFailure`).
//! - Inline formatting (emphasis/links/images) is not in the generation
//!   subset; block text is single-line whitespace-collapsed canonical
//!   text or the typed `InvalidSpec`.
//! - The census counts code blocks (fenced or indented - comrak's
//!   canonical formatter renders a bare fence as an indented block, so
//!   both spellings land identically; the info-string first token is
//!   the language key, `""` when bare), GFM tables (extension on),
//!   lists (bullet or ordered), headings in document order, and
//!   paragraphs anywhere in the tree (a tight list item's inner
//!   paragraph counts - the spec->tree projection predicts exactly the
//!   same shape the canonical writer emits).

use austringer_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::projection::{ProjectionContext, ProjectionProvider};

mod extract;
mod generate;
mod spec;
mod tree;
mod verify;

pub use extract::extract;
pub use generate::{generate, render_html};
pub use spec::{Block, MAX_BLOCKS, MdSpec};
pub use tree::MdTree;
pub use verify::{profile, verify};

/// Producer-disclosure engine name for every markdown op.
pub const ENGINE: &str = "austringer-media::text::markdown";

/// The markdown text-projection bound: 1200 CHARS (the M1 ceiling is
/// 2000; the outline projection stays deliberately lower).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 1200;

const _: () = assert!(TEXT_PROJECTION_MAX_CHARS <= crate::extract::TEXT_PROJECTION_MAX_CHARS);

/// The markdown deterministic projector (M10 seam): a thin wrap over
/// the extractor's bounded outline projection - same bytes, same
/// projection, never a fabricated transcript.
pub struct MarkdownProjector;

impl ProjectionProvider for MarkdownProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        extract::extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}
