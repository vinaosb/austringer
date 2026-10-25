//! The png/apng writer + the INDEPENDENT spec->tree projections (plan
//! M28): `png`-crate encoder with `set_animated`, 16-bit depth, the
//! eXIf chunk (kamadak TIFF payload) and the iTXt XMP packet, plus the
//! explicit sRGB chunk (generated images assume sRGB - arbitrary ICC
//! embedding is not a v0.2 generation feature). Determinism: the png
//! pipeline has no clock fields, so byte-determinism holds
//! unconditionally.

use png::{BitDepth, Encoder};
use serde_json::{Value, json};

use super::meta;
use super::pixels;
use super::png::{color_name_for, png_color};
use super::spec::ImageSpec;
use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Generate deterministic png/apng bytes from a validated spec.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec violates the png contract
/// (see [`ImageSpec::check`]); [`MediaError::ParseFailure`] on encoder
/// stream failure.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let image = ImageSpec::parse(spec, FormatId::Png)?;
    image.check(FormatId::Png)?;
    let mut out = Vec::new();
    {
        let mut encoder = Encoder::new(&mut out, image.width, image.height);
        encoder.set_color(png_color(image.color));
        encoder.set_depth(if image.bit_depth == 16 {
            BitDepth::Sixteen
        } else {
            BitDepth::Eight
        });
        // Explicit sRGB assumption (digest fidelity note: embed iCCP or
        // assume sRGB - generation assumes sRGB).
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        if let Some(animated) = image.animated {
            encoder
                .set_animated(animated.num_frames, animated.num_plays)
                .and_then(|()| encoder.set_frame_delay(animated.delay_num, animated.delay_den))
                .map_err(|_| MediaError::InvalidSpec)?;
        }
        encoder.validate_sequence(true);
        if let Some(xmp) = &image.xmp
            && xmp.has_fields()
        {
            encoder
                .add_itxt_chunk(meta::XMP_KEYWORD.to_owned(), meta::xmp_packet(xmp))
                .map_err(|_| MediaError::InvalidSpec)?;
        }
        let mut writer = encoder
            .write_header()
            .map_err(|_| MediaError::ParseFailure)?;
        if let Some(exif) = &image.exif
            && exif.has_fields()
        {
            let tiff = meta::exif_tiff(exif)?;
            writer
                .write_chunk(png::chunk::eXIf, &tiff)
                .map_err(|_| MediaError::ParseFailure)?;
        }
        for frame in 0..image.frames() {
            writer
                .write_image_data(&pixels::render_frame(&image, frame))
                .map_err(|_| MediaError::ParseFailure)?;
        }
        writer.finish().map_err(|_| MediaError::ParseFailure)?;
    }
    Ok(out)
}

/// The INDEPENDENT spec->tree projection at the METADATA level: the
/// canonical extract tree this spec's document must extract to (M5
/// provenance rule - computed from the spec, never by extracting the
/// writer's own output).
#[must_use]
pub fn spec_tree(spec: &ImageSpec) -> Value {
    let exif = spec
        .exif
        .as_ref()
        .filter(|exif| exif.has_fields())
        .map(|exif| {
            meta::exif_tiff(exif)
                .ok()
                .and_then(|tiff| meta::parse_exif_tree(&tiff))
                .unwrap_or_else(|| json!({"fields": {}, "unknown_tags": 0}))
        });
    let xmp = spec
        .xmp
        .as_ref()
        .filter(|xmp| xmp.has_fields())
        .map(|xmp| json!(meta::bound_xmp(&meta::xmp_packet(xmp))));
    json!({
        "format": "png",
        "width": spec.width,
        "height": spec.height,
        "bit_depth": spec.bit_depth,
        "color_type": color_name_for(spec.color),
        "interlaced": false,
        "icc_profile": Value::Null,
        "exif": exif,
        "xmp": xmp,
        "animated": super::png::animated_tree(spec),
    })
}

/// The INDEPENDENT spec->tree projection at the VERIFY level: per-frame
/// pixel hashes (rendered from the pattern - the decode-back reference),
/// delays, and play count. Verify diffs its decode-back tree against
/// THIS, exactly.
#[must_use]
pub fn verify_tree(spec: &ImageSpec) -> Value {
    let frames: Vec<Value> = (0..spec.frames())
        .map(|frame| {
            json!({
                "delay": spec.animated.map(|animated| {
                    json!({"delay_num": animated.delay_num, "delay_den": animated.delay_den})
                }),
                "hash": pixels::frame_hash(spec, frame),
            })
        })
        .collect();
    json!({
        "width": spec.width,
        "height": spec.height,
        "bit_depth": spec.bit_depth,
        "color_type": color_name_for(spec.color),
        "num_plays": spec.animated.map_or(0, |animated| animated.num_plays),
        "frames": frames,
    })
}
