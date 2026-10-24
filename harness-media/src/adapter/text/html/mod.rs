//! # HTML5 adapter, pure tier (plan M30)
//!
//! maud generates the self-contained single file (the digest
//! interchange note: html is lossy as interchange - no pagination, no
//! JS execution - so the writer emits honest structural html only);
//! html5gum reparses it for the tag-census extract and the round-trip
//! verify.
//!
//! ## Honesty rows
//!
//! - Licensing (digest 3.1): WHATWG Living Standard, `text/html`
//!   (RFC 2854), GREEN RF.
//! - Generation is self-contained: `<!DOCTYPE html>` +
//!   `<meta charset="utf-8">`, one `<title>` when the spec names one,
//!   no external assets (no src/href/link/script) - test-pinned.
//! - Extraction is a TOKENIZER census (html5gum), not a DOM build:
//!   the WHATWG tokenizer is total over utf8 input (error recovery is
//!   the spec), so a doctype-less fragment is a census fact, never a
//!   failure; non-utf8 input is the typed `ParseFailure` (charset
//!   sniffing for non-utf8 encodings is honestly out of scope).
//! - Tag census counts START tags (void/self-closing included); end
//!   tags mirror start tags except for void elements.
//! - Nothing is ever fetched or executed - script content is listed
//!   as text, never run (standing constraint).

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::projection::{ProjectionContext, ProjectionProvider};

mod extract;
mod generate;
mod spec;
mod tree;
mod verify;

pub use extract::extract;
pub use generate::generate;
pub use spec::{HtmlBlock, HtmlSpec, MAX_BLOCKS};
pub use tree::HtmlTree;
pub use verify::{profile, verify};

/// Producer-disclosure engine name for every html op.
pub const ENGINE: &str = "harness-media::text::html";

/// The html text-projection bound: 1200 CHARS (the M1 ceiling is 2000).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 1200;

const _: () = assert!(TEXT_PROJECTION_MAX_CHARS <= crate::extract::TEXT_PROJECTION_MAX_CHARS);

/// The html deterministic projector (M10 seam): a thin wrap over the
/// extractor's bounded census projection.
pub struct HtmlProjector;

impl ProjectionProvider for HtmlProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        extract::extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}
