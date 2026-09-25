//! JPEG adapter (plan M28): zune-jpeg decodes (dims + pixel decode-back
//! for the PSNR gate), jpeg-encoder encodes (quality/progressive spec),
//! a bounded marker walk surfaces EXIF APP1 / XMP APP1 / progressive
//! SOF, and generation splices EXIF/XMP APP1 segments after SOI (the
//! encoder has no metadata surface). Lossy verify (dims + PSNR floor,
//! never a hash) lives in [`super::jpeg_verify`].

use serde_json::{Value, json};

use super::pixels;
use super::spec::ImageSpec;
use super::{check_deadline, deadline_started, meta};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;

pub use super::jpeg_verify::{diff_profile, verify, verify_tree};

/// Producer-disclosure engine name for every jpeg op.
pub const ENGINE: &str = "harness-media::image::jpeg";

const SOI: u8 = 0xD8;
const SOS: u8 = 0xDA;
const APP1: u8 = 0xE1;
const MAX_SEGMENTS: usize = 4096;

struct JpegMeta {
    progressive: bool,
    exif: Option<Vec<u8>>,
    xmp: Option<String>,
}

fn walk_markers(bytes: &[u8]) -> Result<JpegMeta, MediaError> {
    let mut meta = JpegMeta {
        progressive: false,
        exif: None,
        xmp: None,
    };
    if bytes.first() != Some(&0xFF) || bytes.get(1) != Some(&SOI) {
        return Err(MediaError::ParseFailure);
    }
    let mut cursor = 2_usize;
    let mut segments = 0_usize;
    while cursor + 4 <= bytes.len() {
        segments += 1;
        if segments > MAX_SEGMENTS {
            return Err(MediaError::ExtractTooLarge);
        }
        if bytes[cursor] != 0xFF {
            return Err(MediaError::ParseFailure);
        }
        let marker = bytes[cursor + 1];
        if marker == SOS || marker == 0xD9 {
            break;
        }
        let length = u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
        if length < 2 || cursor + 2 + length > bytes.len() {
            return Err(MediaError::ParseFailure);
        }
        let payload = &bytes[cursor + 4..cursor + 2 + length];
        if marker == APP1 && payload.starts_with(b"Exif\0\0") && meta.exif.is_none() {
            meta.exif = Some(payload[6..].to_vec());
        }
        if marker == APP1 && payload.starts_with(meta::JPEG_XMP_NS) && meta.xmp.is_none() {
            meta.xmp =
                Some(String::from_utf8_lossy(&payload[meta::JPEG_XMP_NS.len()..]).into_owned());
        }
        // SOF2 (0xC2) is the progressive scan header.
        if marker == 0xC2 {
            meta.progressive = true;
        }
        cursor += 2 + length;
    }
    Ok(meta)
}

pub(super) fn decoder(
    bytes: &[u8],
) -> Result<zune_jpeg::JpegDecoder<zune_jpeg::zune_core::bytestream::ZCursor<&[u8]>>, MediaError> {
    use zune_jpeg::zune_core::bytestream::ZCursor;
    let mut decoder = zune_jpeg::JpegDecoder::new(ZCursor::new(bytes));
    decoder
        .decode_headers()
        .map_err(|_| MediaError::ParseFailure)?;
    Ok(decoder)
}
/// Extract dims/color/progressive + EXIF/XMP presence under caller
/// budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-jpeg/truncated bytes;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = deadline_started();
    check_deadline(started, &budgets)?;
    let meta = walk_markers(bytes)?;
    let decode = decoder(bytes)?;
    let info = decode.info().ok_or(MediaError::ParseFailure)?;
    let exif = meta.exif.as_ref().map(|tiff| {
        meta::parse_exif_tree(tiff).unwrap_or_else(|| json!({"fields": {}, "unknown_tags": 0}))
    });
    let tree = json!({
        "format": "jpeg",
        "width": info.width,
        "height": info.height,
        "bit_depth": 8,
        "color_type": if info.components == 1 { "gray" } else { "rgb" },
        "progressive": meta.progressive,
        "exif": exif,
        "xmp": meta.xmp.as_ref().map(|xmp| json!(meta::bound_xmp(xmp))),
    });
    let projection = super::truncate_chars(
        format!(
            "jpeg {}x{} {} 8-bit; progressive={}; exif={} xmp={}",
            tree["width"],
            tree["height"],
            tree["color_type"].as_str().unwrap_or_default(),
            meta.progressive,
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

/// Generate deterministic jpeg bytes (quality/progressive from the
/// spec; EXIF/XMP APP1 segments spliced after SOI when present).
///
/// # Errors
/// [`MediaError::InvalidSpec`] on contract violations;
/// [`MediaError::ParseFailure`] on encoder failure.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let image = ImageSpec::parse(spec, FormatId::Jpeg)?;
    image.check(FormatId::Jpeg)?;
    let samples = pixels::render_frame(&image, 0);
    let quality = image.quality.round().clamp(1.0, 100.0) as u8;
    let mut body = Vec::new();
    {
        let mut encoder = jpeg_encoder::Encoder::new(&mut body, quality);
        encoder.set_progressive(image.progressive);
        let color = match image.color {
            super::spec::ColorKind::Gray => jpeg_encoder::ColorType::Luma,
            _ => jpeg_encoder::ColorType::Rgb,
        };
        encoder
            .encode(
                &samples,
                u16::try_from(image.width).map_err(|_| MediaError::InvalidSpec)?,
                u16::try_from(image.height).map_err(|_| MediaError::InvalidSpec)?,
                color,
            )
            .map_err(|_| MediaError::ParseFailure)?;
    }
    let mut app1 = Vec::new();
    if let Some(exif) = &image.exif
        && exif.has_fields()
    {
        let tiff = meta::exif_tiff(exif)?;
        push_app1(
            &mut app1,
            &[b"Exif\0\0".as_slice(), tiff.as_slice()].concat(),
        )?;
    }
    if let Some(xmp) = &image.xmp
        && xmp.has_fields()
    {
        let packet = meta::xmp_packet(xmp).into_bytes();
        push_app1(&mut app1, &[meta::JPEG_XMP_NS, packet.as_slice()].concat())?;
    }
    if app1.is_empty() {
        return Ok(body);
    }
    app1.extend_from_slice(&body[2..]);
    let mut file = vec![0xFF, SOI];
    file.extend_from_slice(&app1);
    Ok(file)
}

fn push_app1(out: &mut Vec<u8>, payload: &[u8]) -> Result<(), MediaError> {
    let length = payload
        .len()
        .checked_add(2)
        .filter(|total| *total <= u16::MAX as usize)
        .ok_or(MediaError::InvalidSpec)?;
    let length = u16::try_from(length).map_err(|_| MediaError::InvalidSpec)?;
    out.extend_from_slice(&[0xFF, APP1]);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

/// The INDEPENDENT spec->tree projection at the metadata level.
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
        "format": "jpeg",
        "width": spec.width,
        "height": spec.height,
        "bit_depth": 8,
        "color_type": if spec.channels() == 1 { "gray" } else { "rgb" },
        "progressive": spec.progressive,
        "exif": exif,
        "xmp": xmp,
    })
}
