//! The image-family generation-spec schema (plan M28): one typed subset
//! drives png/apng, jpeg, webp-lossless, and avif writers. Parsed
//! strictly - a spec that does not satisfy the contract is the typed
//! `InvalidSpec`, never a guessed default. Per-format rules (jpeg has no
//! alpha/16-bit/animation; webp encode is lossless-only; avif is 8-bit)
//! live in the per-format check functions so the shared schema stays
//! family-wide.

use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

use super::MAX_DIMENSION;

/// The color kinds the family writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorKind {
    /// 3 samples (png/jpeg/webp/avif).
    Rgb,
    /// 4 samples (png/webp/avif; jpeg refuses).
    Rgba,
    /// 1 sample.
    Gray,
    /// 2 samples (png/webp).
    GrayAlpha,
}

/// The deterministic patterns a spec may drive (the round-trip corpora
/// render from these; each animation frame shifts `x` by one `cell` so
/// APNG frame hashes differ honestly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternKind {
    /// Horizontal lerp `color_a` -> `color_b` across the width.
    Gradient,
    /// `color_a`/`color_b` cells, `((x + y) / cell) % 2`.
    Checker,
    /// Vertical stripes, `(x / cell) % 2`.
    Stripes,
    /// `color_a` everywhere.
    Solid,
}

/// The pixel pattern spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// Which pattern.
    pub kind: PatternKind,
    /// First color, one sample per channel, each 0..=255 (16-bit output
    /// scales by 257).
    pub color_a: Vec<u16>,
    /// Second color (required by every kind except `solid`, which
    /// ignores it - an absent `color_b` on a non-solid spec is the typed
    /// `InvalidSpec`).
    #[serde(default)]
    pub color_b: Vec<u16>,
    /// Cell size in pixels (checker/stripes), default 8.
    #[serde(default = "default_cell")]
    pub cell: u32,
}

fn default_cell() -> u32 {
    8
}

/// APNG animation parameters (png only; webp/avif/jpec refuse).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Animated {
    /// Frames in the animation, 2..=256.
    pub num_frames: u32,
    /// Play count (0 = infinite).
    pub num_plays: u32,
    /// Frame delay numerator.
    pub delay_num: u16,
    /// Frame delay denominator.
    pub delay_den: u16,
}

/// EXIF fields a spec may embed (kamadak-exif writer; `date_time` is
/// always the pinned determinism epoch, never the ambient clock).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ExifSpec {
    /// Orientation 1..=8.
    #[serde(default)]
    pub orientation: Option<u16>,
    /// ImageDescription.
    #[serde(default)]
    pub description: Option<String>,
    /// Artist.
    #[serde(default)]
    pub artist: Option<String>,
}

/// XMP packet fields (emitted as deterministic XML).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct XmpSpec {
    /// dc:title.
    #[serde(default)]
    pub title: Option<String>,
    /// dc:description.
    #[serde(default)]
    pub description: Option<String>,
}

/// The family-wide generation spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSpec {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Color kind (default rgb).
    #[serde(default = "default_color")]
    pub color: ColorKind,
    /// 8 or 16 (16 is a png-only feature).
    #[serde(default = "default_depth")]
    pub bit_depth: u8,
    /// The pixel pattern (required - no default image).
    pub pattern: Pattern,
    /// APNG parameters (png only).
    #[serde(default)]
    pub animated: Option<Animated>,
    /// EXIF embed (png/jpeg).
    #[serde(default)]
    pub exif: Option<ExifSpec>,
    /// XMP embed (png/jpeg).
    #[serde(default)]
    pub xmp: Option<XmpSpec>,
    /// Encode quality 1..=100 (jpeg integer, avif float mapped to 1..=100).
    #[serde(default = "default_quality")]
    pub quality: f64,
    /// Progressive JPEG scan layout.
    #[serde(default)]
    pub progressive: bool,
    /// ravif speed 1 (slowest) ..= 10 (fastest).
    #[serde(default = "default_speed")]
    pub speed: u8,
    /// Lossy-verify PSNR floor in dB (jpeg; default
    /// [`super::DEFAULT_PSNR_FLOOR_DB`]).
    #[serde(default)]
    pub psnr_floor_db: Option<f64>,
}

fn default_color() -> ColorKind {
    ColorKind::Rgb
}

fn default_depth() -> u8 {
    8
}

fn default_quality() -> f64 {
    90.0
}

fn default_speed() -> u8 {
    9
}

impl ImageSpec {
    /// Parse a generation spec addressed to `format` and enforce the
    /// family-wide contract. Per-format rules are the format module's
    /// `check` call (see [`ImageSpec::check`]).
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, or a semantic violation (zero/oversized dimensions, bad
    /// color arity or range, bad depth, single-frame animation,
    /// out-of-range quality/speed).
    pub fn parse(spec: &GenerationSpec, format: FormatId) -> Result<Self, MediaError> {
        if spec.format != format {
            return Err(MediaError::InvalidSpec);
        }
        let parsed: Self =
            serde_json::from_value(spec.params.clone()).map_err(|_| MediaError::InvalidSpec)?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// Enforce the per-format rules on top of the parsed spec.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] when a family field is illegal for
    /// this format (alpha/16-bit/animation on jpeg; animation or 16-bit
    /// on webp/avif; jpeg's 65535 dimension ceiling).
    pub fn check(&self, format: FormatId) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        match format {
            FormatId::Jpeg => {
                if self.color == ColorKind::Rgba
                    || self.color == ColorKind::GrayAlpha
                    || self.bit_depth != 8
                    || self.animated.is_some()
                    || self.width > 65_535
                    || self.height > 65_535
                {
                    return Err(invalid());
                }
            }
            FormatId::Webp | FormatId::Avif => {
                if self.bit_depth != 8 || self.animated.is_some() {
                    return Err(invalid());
                }
            }
            FormatId::Png => {}
            _ => return Err(invalid()),
        }
        Ok(())
    }

    /// Samples per pixel for the color kind.
    #[must_use]
    pub fn channels(&self) -> usize {
        match self.color {
            ColorKind::Rgb => 3,
            ColorKind::Rgba => 4,
            ColorKind::Gray => 1,
            ColorKind::GrayAlpha => 2,
        }
    }

    /// Animation frame count (1 when static).
    #[must_use]
    pub fn frames(&self) -> u32 {
        self.animated.map_or(1, |animated| animated.num_frames)
    }

    fn validate(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        if self.width == 0
            || self.height == 0
            || self.width > MAX_DIMENSION
            || self.height > MAX_DIMENSION
        {
            return Err(invalid());
        }
        if self.bit_depth != 8 && self.bit_depth != 16 {
            return Err(invalid());
        }
        let channels = self.channels();
        // Solid ignores color_b, so it may be absent; every other kind
        // needs both colors with full arity and range.
        let colors: &[&Vec<u16>] = if self.pattern.kind == PatternKind::Solid {
            &[&self.pattern.color_a]
        } else {
            &[&self.pattern.color_a, &self.pattern.color_b]
        };
        for color in colors {
            if color.len() != channels || color.iter().any(|sample| *sample > 255) {
                return Err(invalid());
            }
        }
        if self.pattern.cell == 0 || self.pattern.cell > MAX_DIMENSION {
            return Err(invalid());
        }
        if let Some(animated) = self.animated
            && (animated.num_frames < 2 || animated.num_frames > 256 || animated.delay_den == 0)
        {
            return Err(invalid());
        }
        if !(1.0..=100.0).contains(&self.quality) || !(1..=10).contains(&self.speed) {
            return Err(invalid());
        }
        if let Some(orientation) = self.exif.as_ref().and_then(|exif| exif.orientation)
            && !(1..=8).contains(&orientation)
        {
            return Err(invalid());
        }
        if let Some(floor) = self.psnr_floor_db
            && !(1.0..=99.0).contains(&floor)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

impl ExifSpec {
    /// Whether any field would be embedded (an all-None spec embeds
    /// nothing - jpeg treats that as absent, not as an empty APP1).
    #[must_use]
    pub fn has_fields(&self) -> bool {
        self.orientation.is_some() || self.description.is_some() || self.artist.is_some()
    }
}

impl XmpSpec {
    /// Whether any field would be embedded.
    #[must_use]
    pub fn has_fields(&self) -> bool {
        self.title.is_some() || self.description.is_some()
    }
}
