//! Deterministic pixel materialization: the shared pure functions that
//! turn a spec's pattern into sample bytes. The writer, the INDEPENDENT
//! verify-tree projection, and the lossy PSNR reference all render
//! through these functions - one definition of the spec's pixels, three
//! consumers, no drift. Frame N shifts `x` by `N * cell` so APNG frame
//! hashes differ honestly.

use super::spec::{ImageSpec, PatternKind};
use crate::error::MediaError;

/// Render one animation frame's sample bytes (8-bit: one byte per
/// sample; 16-bit: two big-endian bytes per sample - the png crate's
/// native layout).
#[must_use]
pub fn render_frame(spec: &ImageSpec, frame: u32) -> Vec<u8> {
    let channels = spec.channels();
    let width = spec.width as usize;
    let height = spec.height as usize;
    let cell = spec.pattern.cell.max(1) as usize;
    let shift = frame as usize * cell;
    let mut samples = Vec::with_capacity(width * height * channels * spec.bit_depth as usize / 8);
    for y in 0..height {
        for x in 0..width {
            let effective_x = x + shift;
            let color = sample_color(spec, effective_x, y, cell);
            push_color(&mut samples, &color, spec.bit_depth);
        }
    }
    samples
}

/// blake3 hex digest of a rendered frame - the lossless verify hook
/// (decode-back + pixel hash; jpeg never uses this).
#[must_use]
pub fn frame_hash(spec: &ImageSpec, frame: u32) -> String {
    blake3::hash(&render_frame(spec, frame))
        .to_hex()
        .to_string()
}

/// Peak-signal-to-noise ratio in dB between two sample buffers of equal
/// length. `None` when the buffers are empty or mismatched (the caller
/// turns that into a verify failure, never a silent pass).
#[must_use]
pub fn psnr_db(reference: &[u8], actual: &[u8]) -> Option<f64> {
    if reference.is_empty() || reference.len() != actual.len() {
        return None;
    }
    let squared: u64 = reference
        .iter()
        .zip(actual)
        .map(|(expected, got)| {
            let delta = i32::from(*expected) - i32::from(*got);
            u64::try_from(delta * delta).unwrap_or(0)
        })
        .sum();
    if squared == 0 {
        return Some(99.0);
    }
    let mean = squared as f64 / reference.len() as f64;
    Some(10.0 * (255.0_f64 * 255.0 / mean).log10())
}

fn sample_color(spec: &ImageSpec, x: usize, y: usize, cell: usize) -> Vec<u16> {
    let pattern = &spec.pattern;
    match pattern.kind {
        PatternKind::Solid => pattern.color_a.clone(),
        PatternKind::Stripes => {
            if (x / cell).is_multiple_of(2) {
                pattern.color_a.clone()
            } else {
                pattern.color_b.clone()
            }
        }
        PatternKind::Checker => {
            let phase = (x / cell + y / cell) % 2;
            if phase == 0 {
                pattern.color_a.clone()
            } else {
                pattern.color_b.clone()
            }
        }
        PatternKind::Gradient => {
            let span = spec.width.saturating_sub(1).max(1) as f64;
            let t = (x % spec.width as usize) as f64 / span;
            pattern
                .color_a
                .iter()
                .zip(&pattern.color_b)
                .map(|(a, b)| {
                    let lerped = f64::from(*a) + (f64::from(*b) - f64::from(*a)) * t;
                    lerped.round().clamp(0.0, 255.0) as u16
                })
                .collect()
        }
    }
}

fn push_color(out: &mut Vec<u8>, color: &[u16], bit_depth: u8) {
    if bit_depth == 16 {
        for sample in color {
            let wide = *sample * 257;
            out.extend_from_slice(&wide.to_be_bytes());
        }
    } else {
        for sample in color {
            out.push(u8::try_from(*sample).unwrap_or(0));
        }
    }
}

/// Rendered byte length of one frame (the decode-back size check).
#[must_use]
pub fn frame_len(spec: &ImageSpec) -> usize {
    spec.width as usize * spec.height as usize * spec.channels() * spec.bit_depth as usize / 8
}

/// The budget pre-flight every decoder-side render runs: a produced
/// image whose declared dimensions exceed the caller's total-size cap
/// fails typed before any allocation.
///
/// # Errors
/// [`MediaError::ExtractTooLarge`] when `width x height x channels`
/// exceeds `max_total_bytes`.
pub fn guard_dimensions(
    width: u32,
    height: u32,
    channels: usize,
    max_total_bytes: u64,
) -> Result<(), MediaError> {
    let needed = u64::from(width) * u64::from(height) * channels as u64;
    if needed > max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    Ok(())
}
