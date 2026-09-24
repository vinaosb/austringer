//! # SVG adapter, pure tier (plan M11) - the registration pattern
//!
//! The first real adapter: quick-xml pulls the bounded structure walk,
//! the hand-pinned writer emits deterministic SVG 1.1, and the M5 diff +
//! usvg strict parse gate the round trip. The registration shape landed
//! with this module (wiring.rs per-op bindings + descriptor-claim flips
//! in the M2 table + [`ENGINE`] for producer disclosure + the M10
//! projector) is mirrored by every later adapter todo (M12-M23,
//! M28-M30).
//!
//! ## Honesty rows (surfaced verbatim by the M27 matrix)
//!
//! - Licensing (digest 3.2 SVG row): W3C SVG 1.1 2nd ed. (2011), MIME
//!   `image/svg+xml`, GREEN under the W3C RF policy; SVG 2 is still CR -
//!   generation targets 1.1 semantics exclusively, so no SVG2-only
//!   feature is emitted and no fallback is owed.
//! - Thumbnails: NONE in v0.2 - rasterization is explicitly out of
//!   scope (the deferred resvg stage), never a stub.
//! - Stage-2 visual verification (resvg render -> perceptual hash):
//!   named deferred; stage-1 structural validation via usvg strict
//!   parse is the shipped gate.
//! - svgz (gzip-wrapped svg): probed and listed, never decoded in
//!   v0.2 - extraction of gzip bytes fails typed `ParseFailure`.
//! - Fonts: generated `<text>` claims only common font stacks
//!   (default `sans-serif`); font availability is the #1 render
//!   divergence and stays a render-stage concern.
//! - Determinism: svg has no clock fields, so byte-determinism holds
//!   unconditionally (pinned by a byte-identical repeat test).
//!
//! ## Security posture
//!
//! XML external entities are the classic parser attack surface: any
//! `<!DOCTYPE>` is refused typed (quick-xml never expands custom
//! entities, so billion-laughs cannot begin), and general references
//! resolve only for the five predefined XML names and numeric character
//! references. Nothing is ever fetched - raster `href`s are listed.

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::projection::{ProjectionContext, ProjectionProvider};

mod extract;
mod generate;
mod shapes;
mod spec;
mod tree;
mod verify;

pub use extract::extract;
pub use generate::generate;
pub use shapes::{CommonAttrs, Shape, DEFAULT_FONT_FAMILY};
pub use spec::{SvgSpec, MAX_SHAPES};
pub use verify::{profile, verify};

/// Producer-disclosure engine name for every svg op (extract, generate,
/// verify; probe stays under the shared sniffing engine).
pub const ENGINE: &str = "harness-media::svg";

/// The svg per-format text-projection bound: 1200 CHARS (the M1 char
/// ceiling is 2000; the svg default is deliberately lower).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 1200;

/// The SVG 1.1 namespace every generated root declares.
pub const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// The svg deterministic projector (M10 seam): a thin wrap over the
/// extractor's bounded text projection - same bytes, same projection,
/// never a fabricated transcript.
pub struct SvgProjector;

impl ProjectionProvider for SvgProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(
        &self,
        bytes: &[u8],
        context: &ProjectionContext<'_>,
    ) -> Result<String, MediaError> {
        extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}
