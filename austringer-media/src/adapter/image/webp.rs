//! WebP adapter (plan M28): `image-webp` on both sides - LOSSLESS
//! generation only. Lossy and animated encode are the typed M17 ffmpeg
//! redirect (the C libwebp path is the digest's MSVC exception lane;
//! libwebp CVE-2023-4863 is the operational reason the Rust decoder is
//! preferred). Verify = decode-back + pixel hash (lossless discipline).

use std::io::Cursor;

use image_webp::{ColorType, WebPDecoder, WebPEncoder};
use serde_json::{Value, json};

use super::pixels;
use super::spec::ImageSpec;
use super::{check_deadline, deadline_started, meta};
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Producer-disclosure engine name for every webp op.
pub const ENGINE: &str = "austringer-media::image::webp";

/// The honesty-row reason lossy/animated webp encode redirects to the
/// ffmpeg subprocess lane.
pub const ENCODE_REDIRECT: &str = "lossy and animated WebP encode is the C libwebp path \
     (the digest's MSVC exception lane): encoding routes to the ffmpeg subprocess lane (M17); \
     the pure tier encodes lossless only";

/// Extract dims/alpha/lossless/animation + ICC/EXIF/XMP metadata.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-webp/truncated bytes;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = deadline_started();
    check_deadline(started, &budgets)?;
    let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(|_| MediaError::ParseFailure)?;
    let (width, height) = decoder.dimensions();
    pixels::guard_dimensions(width, height, 4, budgets.max_total_bytes)?;
    let animated = decoder
        .is_animated()
        .then(|| json!({"num_frames": decoder.num_frames()}));
    let icc = decoder.icc_profile().ok().flatten();
    let exif = decoder.exif_metadata().ok().flatten();
    let xmp = decoder.xmp_metadata().ok().flatten();
    let tree = json!({
        "format": "webp",
        "width": width,
        "height": height,
        "bit_depth": 8,
        "alpha": decoder.has_alpha(),
        "lossless": !decoder.is_lossy(),
        "animated": animated,
        "icc_profile": icc.as_ref().map(|icc| json!({"bytes": icc.len()})),
        "exif": exif.as_ref().map(|tiff| {
            meta::parse_exif_tree(tiff).unwrap_or_else(|| json!({"fields": {}, "unknown_tags": 0}))
        }),
        "xmp": xmp.as_ref().map(|packet| {
            json!(meta::bound_xmp(&String::from_utf8_lossy(packet)))
        }),
    });
    let projection = super::truncate_chars(
        format!(
            "webp {}x{} 8-bit; alpha={} lossless={}; exif={} xmp={}",
            width,
            height,
            tree["alpha"].as_bool().unwrap_or(false),
            tree["lossless"].as_bool().unwrap_or(false),
            tree["exif"].is_object(),
            tree["xmp"].is_string(),
        ),
        super::TEXT_PROJECTION_MAX_CHARS,
    );
    Ok(Extracted {
        tree,
        text_projection: projection,
        thumbnails: Vec::new(),
    })
}

/// Generate LOSSLESS webp bytes from a validated spec. A spec carrying
/// `"lossy": true` or an `animated` block is the typed M17 redirect -
/// never a fake pure-Rust lossy claim.
///
/// # Errors
/// [`MediaError::UnsupportedOperation`] on the lossy/animated redirect;
/// [`MediaError::InvalidSpec`] on contract violations;
/// [`MediaError::ParseFailure`] on encoder failure.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    if spec
        .params
        .get("lossy")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
        || spec
            .params
            .get("animated")
            .is_some_and(|value| !value.is_null())
    {
        return Err(MediaError::UnsupportedOperation {
            format: FormatId::Webp,
            op: crate::descriptor::Operation::Generate,
            reason: ENCODE_REDIRECT.to_owned(),
        });
    }
    let image = ImageSpec::parse(spec, FormatId::Webp)?;
    image.check(FormatId::Webp)?;
    let samples = pixels::render_frame(&image, 0);
    let color = match image.color {
        super::spec::ColorKind::Rgb => ColorType::Rgb8,
        super::spec::ColorKind::Rgba => ColorType::Rgba8,
        super::spec::ColorKind::Gray => ColorType::L8,
        super::spec::ColorKind::GrayAlpha => ColorType::La8,
    };
    let mut out = Vec::new();
    WebPEncoder::new(&mut out)
        .encode(&samples, image.width, image.height, color)
        .map_err(|_| MediaError::ParseFailure)?;
    Ok(out)
}

/// The INDEPENDENT spec->tree projection at the metadata level (the
/// fields a lossless single-frame encode can promise).
#[must_use]
pub fn spec_tree(spec: &ImageSpec) -> Value {
    json!({
        "format": "webp",
        "width": spec.width,
        "height": spec.height,
        "bit_depth": 8,
        "alpha": matches!(spec.color, super::spec::ColorKind::Rgba | super::spec::ColorKind::GrayAlpha),
        "lossless": true,
        "animated": Value::Null,
        "icc_profile": Value::Null,
        "exif": Value::Null,
        "xmp": Value::Null,
    })
}

/// The INDEPENDENT spec->tree projection at the verify level: dims +
/// alpha + the pixel hash of the reference rendered in the DECODER's
/// RGB(A) layout (webp lossless has no gray wire form - L8/La8 inputs
/// are expanded to RGB(A) by the codec).
#[must_use]
pub fn verify_tree(spec: &ImageSpec) -> Value {
    json!({
        "width": spec.width,
        "height": spec.height,
        "alpha": matches!(spec.color, super::spec::ColorKind::Rgba | super::spec::ColorKind::GrayAlpha),
        "hash": blake3::hash(&rgb_like_reference(spec)).to_hex().to_string(),
    })
}

/// Render frame 0 in the decoder's RGB(A) layout: gray specs have their
/// luminance replicated across r/g/b (alpha carried through).
fn rgb_like_reference(spec: &ImageSpec) -> Vec<u8> {
    let samples = pixels::render_frame(spec, 0);
    match spec.color {
        super::spec::ColorKind::Rgb | super::spec::ColorKind::Rgba => samples,
        super::spec::ColorKind::Gray => {
            let mut out = Vec::with_capacity(samples.len() * 3);
            for sample in &samples {
                out.extend_from_slice(&[*sample, *sample, *sample]);
            }
            out
        }
        super::spec::ColorKind::GrayAlpha => samples
            .chunks(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
    }
}

/// The webp diff profile: exact.
#[must_use]
pub fn diff_profile() -> Tolerances {
    Tolerances::default()
}

/// Verify produced webp bytes: decode-back + blake3 pixel hash against
/// the independent projection (lossless discipline - lossy inputs
/// honestly fail the hash).
///
/// # Errors
/// [`MediaError::ParseFailure`] when the bytes do not decode;
/// [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let budgets = ExtractBudgets::default();
    let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(|_| MediaError::ParseFailure)?;
    let (width, height) = decoder.dimensions();
    let channels = if decoder.has_alpha() { 4 } else { 3 };
    pixels::guard_dimensions(width, height, channels, budgets.max_total_bytes)?;
    let frame_len = width as usize * height as usize * channels;
    let mut buffer = vec![0_u8; frame_len];
    decoder
        .read_image(&mut buffer)
        .map_err(|_| MediaError::ParseFailure)?;
    let actual = json!({
        "width": width,
        "height": height,
        "alpha": decoder.has_alpha(),
        "hash": blake3::hash(&buffer).to_hex().to_string(),
    });
    let diff = structural_diff(expected, &actual, &diff_profile())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}
