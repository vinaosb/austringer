//! AVIF adapter, encode lane (plan M28): the pure tier ENCODES via
//! ravif (rav1e, SLOW by design - its own budgets.json timeout, excluded
//! from the <=1 s pure round-trip line) and EXTRACTS the ISOBMFF
//! container (brand, ispe dims, pixi bit depth, alpha auxiliary item) -
//! there is no pure-Rust AV1 decoder (dav1d is C/MSVC-hostile), so
//! pixel decode-back verify is the M17 `#[ignore]` real-ffmpeg tier.
//! Licensing: GREEN with the Sisvel-pool footnote (digest 2/7) - the
//! honesty matrix carries the footnote verbatim.

use serde_json::{Value, json};

use super::spec::ImageSpec;
use super::{check_deadline, deadline_started};
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

/// Producer-disclosure engine name for every avif op.
pub const ENGINE: &str = "austringer-media::image::avif";

/// The honesty note the encode op carries: rav1e is tens-of-seconds
/// slow at default speed and lives outside the <=1 s pure round-trip
/// line (budgets.json `media_format_overrides.avif.encode_timeout_ms`).
pub const ENCODE_NOTE: &str = "ravif/rav1e encode is SLOW by design: it carries its own \
     budgets.json encode timeout and is excluded from the <=1s pure round-trip line; decode-back \
     verification runs through the M17 ffmpeg real-tool tier";

pub use super::avif_write::generate;

/// The avif diff profile: exact.
#[must_use]
pub fn diff_profile() -> Tolerances {
    Tolerances::default()
}

/// Extract the avif container tree (bounded ISOBMFF box walk - brand,
/// ispe dims, pixi bit depth, alpha auxiliary item presence; no AV1
/// decode).
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-avif/truncated containers;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = deadline_started();
    check_deadline(started, &budgets)?;
    let mut walk = ContainerWalk::new(bytes);
    let mut brand = None;
    let mut ispe = None;
    let mut pixi = None;
    let mut alpha = false;
    let mut boxes = 0_u32;
    while let Some(box_type) = walk.next_box()? {
        boxes += 1;
        if boxes > 4096 {
            return Err(MediaError::ExtractTooLarge);
        }
        match &box_type {
            b"ftyp" => {
                let payload = walk.payload();
                // ftyp payload: major_brand (4) + minor_version (4) +
                // compatible brands.
                brand = payload
                    .get(..4)
                    .and_then(|major_brand| std::str::from_utf8(major_brand).ok())
                    .map(str::to_owned);
            }
            b"meta" => {
                let meta = walk.payload();
                walk_meta(meta, &mut ispe, &mut pixi, &mut alpha, started, &budgets)?;
            }
            _ => {}
        }
    }
    let tree = json!({
        "format": "avif",
        "brand": brand,
        "width": ispe.map(|(width, _)| width),
        "height": ispe.map(|(_, height)| height),
        "bit_depth": pixi,
        "alpha": alpha,
    });
    let projection = super::truncate_chars(
        format!(
            "avif brand={} dims={:?} bit_depth={:?} alpha={}; no pure-rust av1 decode - container walk only",
            tree["brand"].as_str().unwrap_or("?"),
            ispe,
            pixi,
            alpha,
        ),
        super::TEXT_PROJECTION_MAX_CHARS,
    );
    Ok(Extracted {
        tree,
        text_projection: projection,
        thumbnails: Vec::new(),
    })
}

/// One-level walk inside `meta` (a full box: 4-byte version/flags) for
/// the iprp property store; ispe/pixi/auxC live in iprp/ipco.
fn walk_meta(
    meta: &[u8],
    ispe: &mut Option<(u64, u64)>,
    pixi: &mut Option<u64>,
    alpha: &mut bool,
    started: std::time::Instant,
    budgets: &ExtractBudgets,
) -> Result<(), MediaError> {
    let mut walk = ContainerWalk::full_box(meta);
    while let Some(box_type) = walk.next_box()? {
        check_deadline(started, budgets)?;
        let payload = walk.payload();
        if box_type == *b"iprp" {
            let mut iprp = ContainerWalk::new(payload);
            while let Some(inner) = iprp.next_box()? {
                if inner == *b"ipco" {
                    walk_ipco(iprp.payload(), ispe, pixi, alpha);
                }
            }
        }
    }
    Ok(())
}

fn walk_ipco(ipco: &[u8], ispe: &mut Option<(u64, u64)>, pixi: &mut Option<u64>, alpha: &mut bool) {
    let mut walk = ContainerWalk::new(ipco);
    while let Some(box_type) = walk.next_box().ok().flatten() {
        let payload = walk.payload();
        if box_type == *b"ispe" && payload.len() >= 12 && ispe.is_none() {
            // ispe full box: version/flags (4) + width (4) + height (4).
            *ispe = Some((be_u32(&payload[4..8]), be_u32(&payload[8..12])));
        }
        if box_type == *b"pixi" && payload.len() >= 6 && pixi.is_none() {
            // pixi full box: version/flags (4) + channel count (1) +
            // per-channel bit depths.
            *pixi = Some(u64::from(payload[5]));
        }
        if box_type == *b"auxC"
            && payload
                .windows(ALPHA_URN.len())
                .any(|window| window == ALPHA_URN)
        {
            *alpha = true;
        }
    }
}

/// The alpha auxiliary-type URN an auxC box carries when the item is an
/// alpha plane (ISO 23000-22 / MIAF).
const ALPHA_URN: &[u8] = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha";

fn be_u32(bytes: &[u8]) -> u64 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).into()
}

/// Minimal bounded ISOBMFF cursor: yields box types and their payloads.
struct ContainerWalk<'a> {
    bytes: &'a [u8],
    cursor: usize,
    header: usize,
}

impl<'a> ContainerWalk<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            cursor: 0,
            header: 0,
        }
    }

    /// A walk whose input carries a full-box prefix (version/flags).
    /// Short inputs become an empty walk.
    fn full_box(bytes: &'a [u8]) -> Self {
        if bytes.len() < 4 {
            return Self {
                bytes,
                cursor: bytes.len(),
                header: bytes.len(),
            };
        }
        Self {
            bytes,
            cursor: 4,
            header: 4,
        }
    }

    fn next_box(&mut self) -> Result<Option<[u8; 4]>, MediaError> {
        if self.cursor + 8 > self.bytes.len() {
            return Ok(None);
        }
        let size = u32::from_be_bytes([
            self.bytes[self.cursor],
            self.bytes[self.cursor + 1],
            self.bytes[self.cursor + 2],
            self.bytes[self.cursor + 3],
        ]) as usize;
        let box_type = [
            self.bytes[self.cursor + 4],
            self.bytes[self.cursor + 5],
            self.bytes[self.cursor + 6],
            self.bytes[self.cursor + 7],
        ];
        let end = if size == 1 {
            // largesize variant: 64-bit size follows the type.
            if self.cursor + 16 > self.bytes.len() {
                return Err(MediaError::ParseFailure);
            }
            let mut wide = [0_u8; 8];
            wide.copy_from_slice(&self.bytes[self.cursor + 8..self.cursor + 16]);
            usize::try_from(u64::from_be_bytes(wide))
                .map_err(|_| MediaError::ParseFailure)?
                .min(self.bytes.len() - self.cursor)
        } else if size == 0 {
            self.bytes.len() - self.cursor
        } else {
            size
        };
        if end < 8 || self.cursor + end > self.bytes.len() {
            return Err(MediaError::ParseFailure);
        }
        self.header = self.cursor + 8;
        self.cursor += end;
        Ok(Some(box_type))
    }

    fn payload(&self) -> &'a [u8] {
        let payload_end = self.cursor.min(self.bytes.len());
        &self.bytes[self.header.min(payload_end)..payload_end]
    }
}

/// The INDEPENDENT spec->tree projection (container-level facts the
/// ravif writer promises: avif brand, ispe dims, pixi 8-bit, alpha from
/// the color kind).
#[must_use]
pub fn spec_tree(spec: &ImageSpec) -> Value {
    json!({
        "format": "avif",
        "brand": "avif",
        "width": spec.width,
        "height": spec.height,
        "bit_depth": 8,
        "alpha": spec.color == super::spec::ColorKind::Rgba,
    })
}

/// Verify produced avif bytes: container extract diffed against the
/// independent projection (the pure gate; pixel decode-back is the
/// `#[ignore]` real-ffmpeg tier).
///
/// # Errors
/// [`MediaError::ParseFailure`] when the container does not parse;
/// [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let diff = structural_diff(expected, &extracted.tree, &diff_profile())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}
