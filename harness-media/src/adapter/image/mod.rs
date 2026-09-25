//! # Image family adapter, pure tier (plan M28)
//!
//! PNG/APNG (W3C Rec 3rd ed - `png` crate, APNG via `set_animated`,
//! 16-bit, eXIf/iTXt chunks), JPEG (zune-jpeg decode + jpeg-encoder
//! encode: quality/progressive), WebP lossless (`image-webp`; lossy and
//! animated encode are the typed M17 ffmpeg redirect - libwebp's C path
//! is the digest's MSVC exception lane), AVIF (`ravif` encode only,
//! under the non-default `avif` feature - SLOW, its own budgets.json
//! timeout, excluded from the <=1 s pure round-trip line; decode-back
//! verify is the M17 `#[ignore]` real tier because no mature pure-Rust
//! AV1 decoder exists - dav1d is C/MSVC-hostile), JXL extract-only
//! (`jxl-oxide`, spec-conforming; generation typed-refused until the
//! digest adoption trigger - the Chrome default flip, expected H2 2026),
//! plus the EXIF/XMP sidecar layer (kamadak-exif R/W; XMP = emitted
//! XML).
//!
//! ## Honesty rows (surfaced verbatim by the M27 matrix)
//!
//! - gif: probe + native vision delivery ONLY - extract and generation
//!   are PERMANENT typed refusals (the ImageBlock path is unchanged).
//! - RED formats (HEIC/HEVC payload, proprietary RAW, FLIF, JPEG 2000,
//!   QOI, DNG): no FormatId exists - probe answers typed Unknown and
//!   generation of them is unrepresentable (test-pinned).
//! - AVIF verdict: GREEN with the Sisvel-pool footnote (digest 2/7).
//! - Color fidelity: generated images ASSUME sRGB (explicit sRGB chunk
//!   on PNG); arbitrary ICC embedding is not a v0.2 generation feature
//!   (extraction reports iCCP presence honestly).
//! - rav1e determinism: single-threaded, asm-free build pins
//!   byte-determinism; the encode timeout is enforced by a detached
//!   worker thread (a timed-out encode finishes and is discarded).
//! - Lossless verify = decode-back + blake3 pixel hash; lossy verify =
//!   dims + PSNR floor, NEVER a hash; APNG verify = frame count +
//!   per-frame durations + per-frame hashes (not just the first frame).
//!
//! ## Licensing (digest 3.2 rows)
//!
//! PNG/APNG GREEN (W3C RF); JPEG GREEN (Forgent assert died 2006);
//! WebP GREEN (irrevocable VP8 grant + MPEG-LA expiry); AVIF GREEN with
//! Sisvel footnote; JXL legally GREEN, adoption reversing. libwebp's
//! CVE-2023-4863 is the operational reason the Rust decoder is
//! preferred.

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::extract::ExtractBudgets;
use crate::format::FormatId;
use crate::projection::{ProjectionContext, ProjectionProvider};

#[cfg(feature = "avif")]
pub mod avif;
#[cfg(feature = "avif")]
mod avif_write;
pub mod jpeg;
mod jpeg_verify;
pub mod jxl;
pub mod meta;
pub mod pixels;
pub mod png;
mod png_write;
pub mod spec;
pub mod webp;

pub use spec::{Animated, ColorKind, ExifSpec, ImageSpec, Pattern, PatternKind, XmpSpec};

pub use meta::{EXIF_DATE_FORMAT, JPEG_XMP_NS, XMP_KEYWORD};
pub use pixels::{frame_hash, render_frame};

/// The image-family per-format text-projection bound: 1200 CHARS (the
/// M1 ceiling is 2000; the family default is deliberately lower).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 1200;

/// Upper bound on either dimension of a generated image (bounded writer
/// output; fixtures stay far below it).
pub const MAX_DIMENSION: u32 = 16_384;

/// The default PSNR floor for lossy (jpeg) verification, in dB. Specs
/// may raise it via `psnr_floor_db`.
pub const DEFAULT_PSNR_FLOOR_DB: f64 = 25.0;

/// Truncate to at most `max` CHARS, at a char boundary (svg lane's
/// helper, restated locally so the family owns its bound).
#[must_use]
pub(crate) fn truncate_chars(raw: String, max: usize) -> String {
    match raw.char_indices().nth(max) {
        Some((index, _)) => raw[..index].to_owned(),
        None => raw,
    }
}

/// The per-format deterministic projector (M10 seam): a thin wrap over
/// the format's extractor - same bytes, same projection, never a
/// fabricated description.
pub struct ImageProjector {
    format: FormatId,
}

/// Build the deterministic projector for one image format.
#[must_use]
pub fn projector(format: FormatId) -> ImageProjector {
    ImageProjector { format }
}

impl ProjectionProvider for ImageProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        let extracted = match self.format {
            FormatId::Png => png::extract(bytes, context.budgets),
            FormatId::Jpeg => jpeg::extract(bytes, context.budgets),
            FormatId::Webp => webp::extract(bytes, context.budgets),
            #[cfg(feature = "avif")]
            FormatId::Avif => avif::extract(bytes, context.budgets),
            FormatId::Jxl => jxl::extract(bytes, context.budgets),
            _ => return Err(MediaError::UnsupportedFormat),
        }?;
        Ok(extracted.text_projection)
    }
}

/// The deadline check every extractor runs per unit of work (png:
/// per frame; jpeg/webp/jxl: per header/pixel pass).
pub(crate) fn deadline_started() -> std::time::Instant {
    std::time::Instant::now()
}

pub(crate) fn check_deadline(
    started: std::time::Instant,
    budgets: &ExtractBudgets,
) -> Result<(), MediaError> {
    if started.elapsed() >= budgets.deadline {
        return Err(MediaError::ExtractTooLarge);
    }
    Ok(())
}
