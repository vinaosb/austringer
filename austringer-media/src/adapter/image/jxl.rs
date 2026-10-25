//! JXL adapter, EXTRACT-ONLY (plan M28): `jxl-oxide` (pure Rust,
//! spec-conforming) surfaces dims, bit depth, XYB encoding, animation
//! ticks, frame count, and aux-box metadata presence. Generation is the
//! typed adoption-gated refusal wired in M6 (promote the day Chrome
//! flips JXL default on - expected H2 2026); verify is refused because
//! no writer exists to gate.

use jxl_oxide::image::BitDepth;
use jxl_oxide::{AuxBoxData, JxlImage};
use serde_json::json;

use super::{check_deadline, deadline_started};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

/// Producer-disclosure engine name for the jxl extract op.
pub const ENGINE: &str = "austringer-media::image::jxl";

const MAX_FRAMES_PROBED: u32 = 4096;

/// Extract the jxl structural tree (header + frame-header walk - no
/// pixel decode) under caller budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-jxl/truncated bytes;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = deadline_started();
    check_deadline(started, &budgets)?;
    let image = JxlImage::builder()
        .read(bytes)
        .map_err(|_| MediaError::ParseFailure)?;
    let header = image.image_header();
    let metadata = &header.metadata;
    let bit_depth = metadata.bit_depth.bits_per_sample();
    let animated = metadata.animation.as_ref().map(|animation| {
        json!({
            "tps_num": animation.tps_numerator,
            "tps_den": animation.tps_denominator,
        })
    });
    let mut frame_count = 0_usize;
    while frame_count < MAX_FRAMES_PROBED as usize {
        check_deadline(started, &budgets)?;
        if image.frame(frame_count).is_none() {
            break;
        }
        frame_count += 1;
    }
    let first_frame_modular = image
        .frame(0)
        .map(|frame| matches!(frame.header().encoding, jxl_oxide::frame::Encoding::Modular))
        .unwrap_or(false);
    let exif_bytes = aux_len(&image, true);
    let xml_bytes = aux_len(&image, false);
    let tree = json!({
        "format": "jxl",
        "width": image.width(),
        "height": image.height(),
        "bit_depth": bit_depth,
        "float_sample": matches!(metadata.bit_depth, BitDepth::FloatSample { .. }),
        "xyb_encoded": metadata.xyb_encoded,
        "animated": animated,
        "frame_count": frame_count,
        "modular": first_frame_modular,
        "exif_bytes": exif_bytes,
        "xml_bytes": xml_bytes,
    });
    let projection = super::truncate_chars(
        format!(
            "jxl {}x{} {}-bit; xyb={}; frames={}; exif_bytes={}; xml_bytes={}",
            tree["width"],
            tree["height"],
            bit_depth,
            metadata.xyb_encoded,
            frame_count,
            exif_bytes.map_or(0, u64::from),
            xml_bytes.map_or(0, u64::from),
        ),
        super::TEXT_PROJECTION_MAX_CHARS,
    );
    Ok(Extracted {
        tree,
        text_projection: projection,
        thumbnails: Vec::new(),
    })
}

/// Aux-box (Exif/XML) payload length or `None`; bare codestreams carry
/// no boxes and invalid payloads are honestly absent here.
fn aux_len(image: &JxlImage, exif: bool) -> Option<u32> {
    let boxes = image.aux_boxes();
    let bytes: Option<&[u8]> = if exif {
        match boxes.first_exif() {
            Ok(AuxBoxData::Data(raw)) => Some(raw.payload()),
            _ => None,
        }
    } else {
        match boxes.first_xml() {
            AuxBoxData::Data(bytes) => Some(bytes),
            _ => None,
        }
    };
    bytes.map(|payload| u32::try_from(payload.len()).unwrap_or(u32::MAX))
}
