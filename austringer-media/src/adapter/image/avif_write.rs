//! The ravif encode lane (plan M28): pure Rust but SLOW - rav1e at
//! default speed takes tens of seconds, so the encode op carries its
//! OWN timeout (mirroring budgets.json's
//! `media_format_overrides.avif.encode_timeout_ms`) enforced by a
//! detached worker thread: a timed-out encode fails the typed
//! [`MediaError::Timeout`] while the worker finishes and is discarded.
//! Determinism: single-threaded, asm-free rav1e (default-features=false
//! drops rayon and the nasm lane) is byte-deterministic - pinned by
//! test.

use std::sync::mpsc;
use std::time::Duration;

use ravif::{BitDepth, Encoder, Img, RGBA8};

use super::pixels;
use super::spec::ImageSpec;
use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// The encode timeout (mirrors austringer-bench budgets.json
/// `media_format_overrides.avif.encode_timeout_ms` = 30000 - the
/// budgets file is the contract of record).
pub const ENCODE_TIMEOUT_MS: u64 = 30_000;

/// Generate avif bytes from a validated spec (8-bit RGB(A), quality
/// 1..=100, ravif speed 1..=10 - 10 fastest).
///
/// # Errors
/// [`MediaError::InvalidSpec`] on contract violations;
/// [`MediaError::Timeout`] when the encode exceeds
/// [`ENCODE_TIMEOUT_MS`]; [`MediaError::ParseFailure`] on encoder
/// failure.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let image = ImageSpec::parse(spec, FormatId::Avif)?;
    image.check(FormatId::Avif)?;
    let samples = pixels::render_frame(&image, 0);
    let rgba = rgba_plane(&image, &samples);
    let width = image.width as usize;
    let height = image.height as usize;
    let quality = image.quality.clamp(1.0, 100.0) as f32;
    let speed = image.speed;
    // Detached worker: rav1e has no internal deadline, so the handler
    // enforces its own (avif::ENCODE_NOTE is the honesty row).
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let buffer = Img::new(rgba.as_slice(), width, height);
        let encoded = Encoder::new()
            .with_quality(quality)
            .with_speed(speed)
            // ravif's default (Auto -> 10-bit) would break the lane's
            // 8-bit contract; the v0.2 avif lane writes 8-bit files.
            .with_bit_depth(BitDepth::Eight)
            .encode_rgba(buffer)
            .map(|encoded| encoded.avif_file);
        let _ = sender.send(encoded);
    });
    match receiver.recv_timeout(Duration::from_millis(ENCODE_TIMEOUT_MS)) {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(_)) => Err(MediaError::ParseFailure),
        Err(_) => Err(MediaError::Timeout),
    }
}

/// Expand the spec's sample layout into ravif's RGBA8 plane (gray
/// replicated across r/g/b; alpha carried through).
fn rgba_plane(spec: &ImageSpec, samples: &[u8]) -> Vec<RGBA8> {
    match spec.color {
        super::spec::ColorKind::Rgba => samples
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| RGBA8::new(pixel[0], pixel[1], pixel[2], pixel[3]))
            .collect(),
        super::spec::ColorKind::Rgb => samples
            .as_chunks::<3>()
            .0
            .iter()
            .map(|pixel| RGBA8::new(pixel[0], pixel[1], pixel[2], 255))
            .collect(),
        super::spec::ColorKind::GrayAlpha => samples
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pixel| RGBA8::new(pixel[0], pixel[0], pixel[0], pixel[1]))
            .collect(),
        super::spec::ColorKind::Gray => samples
            .iter()
            .map(|luma| RGBA8::new(*luma, *luma, *luma, 255))
            .collect(),
    }
}
