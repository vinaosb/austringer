//! PNG/APNG adapter (plan M28): the `png` crate on both sides - decode
//! extracts dims/bit-depth/color-mode/ICC/EXIF presence/animation
//! control (no pixel pass), and verify does the full decode-back with
//! per-frame blake3 hashes plus frame count and durations. Generation
//! lives in [`super::png_write`].

use std::io::Cursor;

use png::{Decoder, Reader};
use serde_json::{Value, json};

use super::pixels;
use super::spec::{ColorKind, ImageSpec};
use super::{TEXT_PROJECTION_MAX_CHARS, check_deadline, deadline_started, meta};
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

/// Producer-disclosure engine name for every png op.
pub const ENGINE: &str = "austringer-media::image::png";

pub use super::png_write::{generate, spec_tree, verify_tree};

/// The png diff profile: exact (hashes are strings, dims integers) - no
/// epsilon, no unordered paths, nothing ignored.
#[must_use]
pub fn diff_profile() -> Tolerances {
    Tolerances::default()
}

fn color_name(color: png::ColorType) -> &'static str {
    match color {
        png::ColorType::Grayscale => "gray",
        png::ColorType::GrayscaleAlpha => "gray_alpha",
        png::ColorType::Rgb => "rgb",
        png::ColorType::Rgba => "rgba",
        png::ColorType::Indexed => "indexed",
    }
}

fn metadata_tree(info: &png::Info) -> Value {
    let exif = info.exif_metadata.as_ref().map(|tiff| {
        meta::parse_exif_tree(tiff).unwrap_or_else(|| json!({"fields": {}, "unknown_tags": 0}))
    });
    let xmp = info
        .utf8_text
        .iter()
        .find(|entry| entry.keyword == meta::XMP_KEYWORD)
        .and_then(|entry| {
            entry
                .get_text()
                .ok()
                .map(|text| json!(meta::bound_xmp(&text)))
        });
    json!({
        "format": "png",
        "width": info.width,
        "height": info.height,
        "bit_depth": info.bit_depth as u8,
        "color_type": color_name(info.color_type),
        "interlaced": info.interlaced,
        "icc_profile": info.icc_profile.as_ref().map(|icc| json!({"bytes": icc.len()})),
        "exif": exif,
        "xmp": xmp,
        "animated": info.animation_control.map(|control| {
            json!({"num_frames": control.num_frames, "num_plays": control.num_plays})
        }),
    })
}

fn projection(tree: &Value) -> String {
    let mut out = format!(
        "png {}x{} {} {}-bit",
        tree["width"], tree["height"], tree["color_type"], tree["bit_depth"]
    );
    if let Some(animated) = tree["animated"].as_object() {
        out.push_str(&format!(
            "; animated frames={} plays={}",
            animated
                .get("num_frames")
                .map(Value::to_string)
                .unwrap_or_default(),
            animated
                .get("num_plays")
                .map(Value::to_string)
                .unwrap_or_default(),
        ));
    }
    out.push_str(&format!(
        "; icc={} exif={} xmp={}",
        tree["icc_profile"].is_object(),
        tree["exif"].is_object(),
        tree["xmp"].is_string()
    ));
    super::truncate_chars(out, TEXT_PROJECTION_MAX_CHARS)
}

/// Extract the png structural tree (headers + chunk metadata, no pixel
/// decode) under caller budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-png/truncated bytes;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = deadline_started();
    check_deadline(started, &budgets)?;
    let limits = png::Limits {
        bytes: usize::try_from(budgets.max_total_bytes).unwrap_or(usize::MAX),
    };
    let decoder = Decoder::new_with_limits(Cursor::new(bytes), limits);
    let reader = decoder.read_info().map_err(|_| MediaError::ParseFailure)?;
    let info = reader.info().clone();
    let tree = metadata_tree(&info);
    Ok(Extracted {
        text_projection: projection(&tree),
        tree,
        thumbnails: Vec::new(),
    })
}

fn frames_tree(
    reader: &mut Reader<Cursor<&[u8]>>,
    budgets: &ExtractBudgets,
) -> Result<(u32, u8, Value), MediaError> {
    let started = deadline_started();
    let info = reader.info().clone();
    let width = info.width;
    let height = info.height;
    let channels = info.color_type.samples();
    let depth = info.bit_depth as u8;
    // Sub-byte depths pack rows: bytes-per-row = ceil(bits / 8).
    let row_bytes = (width as usize * channels * depth as usize).div_ceil(8);
    let frame_len = row_bytes * height as usize;
    pixels::guard_dimensions(
        width,
        height,
        channels * (depth as usize).div_ceil(8),
        budgets.max_total_bytes,
    )?;
    let num_frames = info
        .animation_control
        .map_or(1, |control| control.num_frames);
    let mut frames = Vec::new();
    let mut buffer = vec![0_u8; frame_len];
    for index in 0..num_frames {
        check_deadline(started, budgets)?;
        // png-crate frame contract: next_frame decodes the CURRENT
        // fcTL's frame and never advances; next_frame_info advances to
        // the next fcTL, and read_info has already positioned frame 0's
        // control. The delay is read from the current control before
        // decoding, and the advance happens only while frames remain.
        let delay = reader
            .info()
            .frame_control
            .map(|control| json!({"delay_num": control.delay_num, "delay_den": control.delay_den}));
        reader
            .next_frame(&mut buffer)
            .map_err(|_| MediaError::ParseFailure)?;
        if index + 1 < num_frames {
            reader
                .next_frame_info()
                .map_err(|_| MediaError::ParseFailure)?;
        }
        frames.push(json!({
            "delay": delay,
            "hash": blake3::hash(&buffer).to_hex().to_string(),
        }));
    }
    let num_plays = info
        .animation_control
        .map_or(0, |control| control.num_plays);
    Ok((num_plays, depth, json!({ "frames": frames })))
}

/// Verify produced png bytes against the INDEPENDENT expected tree
/// (from [`verify_tree`]): full decode-back, per-frame pixel hash, frame
/// count + durations, dims/bit-depth - exact diff, no tolerance.
///
/// # Errors
/// [`MediaError::ParseFailure`] when the bytes do not decode;
/// [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let budgets = ExtractBudgets::default();
    let limits = png::Limits {
        bytes: usize::try_from(budgets.max_total_bytes).unwrap_or(usize::MAX),
    };
    let decoder = Decoder::new_with_limits(Cursor::new(bytes), limits);
    let mut reader = decoder.read_info().map_err(|_| MediaError::ParseFailure)?;
    let info = reader.info().clone();
    let (num_plays, _depth, frames) = frames_tree(&mut reader, &budgets)?;
    let actual = json!({
        "width": info.width,
        "height": info.height,
        "bit_depth": info.bit_depth as u8,
        "color_type": color_name(info.color_type),
        "num_plays": num_plays,
        "frames": frames["frames"],
    });
    let diff = structural_diff(expected, &actual, &diff_profile())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}

/// Map the spec color kind onto the png encoder color type.
pub(super) fn png_color(color: ColorKind) -> png::ColorType {
    match color {
        ColorKind::Rgb => png::ColorType::Rgb,
        ColorKind::Rgba => png::ColorType::Rgba,
        ColorKind::Gray => png::ColorType::Grayscale,
        ColorKind::GrayAlpha => png::ColorType::GrayscaleAlpha,
    }
}

/// Map the spec color kind onto its canonical tree name.
pub(super) fn color_name_for(color: ColorKind) -> &'static str {
    match color {
        ColorKind::Rgb => "rgb",
        ColorKind::Rgba => "rgba",
        ColorKind::Gray => "gray",
        ColorKind::GrayAlpha => "gray_alpha",
    }
}

/// Independent spec->tree helper shared by the write-side projections:
/// animation parameters as the extractor reports them.
pub(super) fn animated_tree(spec: &ImageSpec) -> Value {
    spec.animated.map_or(
        Value::Null,
        |animated| json!({"num_frames": animated.num_frames, "num_plays": animated.num_plays}),
    )
}
