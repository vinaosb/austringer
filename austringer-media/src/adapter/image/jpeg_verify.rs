//! The JPEG verify lane (plan M28): LOSSY discipline - dims exact +
//! PSNR floor from a decode-back, NEVER a pixel hash. The reference
//! render comes from the expected tree's pattern block (the diff
//! profile ignores `/pattern` - it is input, not a claim).

use serde_json::{Value, json};

use super::jpeg::decoder;
use super::pixels;
use super::spec::ImageSpec;
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use std::collections::BTreeSet;

/// The INDEPENDENT spec->tree projection at the verify level: dims +
/// PSNR floor (never a hash) + the pattern block the reference render
/// needs (ignored by the diff profile at `/pattern`).
#[must_use]
pub fn verify_tree(spec: &ImageSpec) -> Value {
    json!({
        "width": spec.width,
        "height": spec.height,
        "psnr_floor_db": spec.psnr_floor_db.unwrap_or(super::DEFAULT_PSNR_FLOOR_DB),
        "meets_floor": true,
        "pattern": serde_json::to_value(&spec.pattern).unwrap_or(Value::Null),
    })
}

/// The jpeg diff profile: the `/pattern` reference block is input, not
/// a claim - ignored for the comparison.
#[must_use]
pub fn diff_profile() -> Tolerances {
    let mut ignored_at = BTreeSet::new();
    ignored_at.insert("/pattern".to_owned());
    Tolerances {
        float_epsilon: 1e-9,
        unordered_at: BTreeSet::new(),
        ignored_at,
    }
}

/// Verify produced jpeg bytes: decode-back (zune-jpeg), reference
/// render from the expected tree's pattern, PSNR against the floor -
/// dims exact, no hash, no silent pass.
///
/// # Errors
/// [`MediaError::ParseFailure`] when the bytes or expected tree do not
/// decode/parse; [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let floor = expected["psnr_floor_db"]
        .as_f64()
        .ok_or(MediaError::InvalidSpec)?;
    let reference_pattern: super::spec::Pattern =
        serde_json::from_value(expected["pattern"].clone()).map_err(|_| MediaError::InvalidSpec)?;
    let width = expected["width"].as_u64().unwrap_or(0) as u32;
    let height = expected["height"].as_u64().unwrap_or(0) as u32;
    let mut decode = decoder(bytes)?;
    let samples = decode.decode().map_err(|_| MediaError::ParseFailure)?;
    let info = decode.info().ok_or(MediaError::ParseFailure)?;
    let channels = usize::from(info.components.max(1));
    let reference_spec = ImageSpec {
        width,
        height,
        color: if channels == 1 {
            super::spec::ColorKind::Gray
        } else {
            super::spec::ColorKind::Rgb
        },
        bit_depth: 8,
        pattern: reference_pattern,
        animated: None,
        exif: None,
        xmp: None,
        quality: 90.0,
        progressive: false,
        speed: 9,
        psnr_floor_db: Some(floor),
    };
    let reference = pixels::render_frame(&reference_spec, 0);
    let comparable = (width as usize * height as usize * channels).min(samples.len());
    let meets_floor = pixels::psnr_db(&reference[..comparable], &samples[..comparable])
        .is_some_and(|psnr| psnr >= floor);
    let actual = json!({
        "width": info.width,
        "height": info.height,
        "psnr_floor_db": floor,
        "meets_floor": meets_floor,
    });
    let diff = structural_diff(expected, &actual, &diff_profile())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}
