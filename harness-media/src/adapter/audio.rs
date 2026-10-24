//! # Audio adapter (plan M14: pure decode + WAV/FLAC synth + the
//! transcription seam)
//!
//! Digest section 3.4 verdicts, surfaced verbatim by the M27 matrix:
//! **Opus** GREEN (IETF RFC 6716 + Ogg-Opus RFC 7845, RF) - but
//! symphonia's opus decode is UNIMPLEMENTED (status "-"), so opus
//! decode routes to the M17 ffmpeg subprocess lane, typed in the pure
//! path; **FLAC** GREEN (Xiph BSD-3 + IETF RFC 9639, patent-free,
//! bit-exact); **WAV** RIFF de-facto 1991, zero licensing; **MP3**
//! ISO/IEC 11172-3:1993 with patents verified EXPIRED 2017 (Fraunhofer
//! closed its program); **AAC stays RED** (Via Licensing pool alive) -
//! excluded from every lane of this adapter.
//!
//! ## Honesty rows
//!
//! - Decode (wav/pcm, flac, mp3, ogg/vorbis): symphonia 0.6, bounded
//!   per-packet loop under the caller's `ExtractBudgets`. Integer
//!   normalization is width-exact (symphonia shifts sub-32-bit FLAC to
//!   full i32 scale; the corpus peak pin ~0.125 is the scale guard).
//! - ENCODE claims: WAV via hound (pcm16/32/f32) and FLAC via flacenc
//!   are the ONLY codecs this crate ever encodes. opus/mp3/vorbis
//!   encode is the typed [`Self::lossy_encode_redirect`] pointing at
//!   the M17 ffmpeg lane (libopus/libmp3lame/libvorbis are all GREEN
//!   flags there) - no fake pure-Rust encoder is claimed, and AAC is
//!   never encoded anywhere in this workspace.
//! - Transcripts are MODEL-MEDIATED ONLY (the M10 pipeline): this
//!   adapter registers a deterministic TextProjection projector (the
//!   pinned one-line summary) and NO Transcript projector - a
//!   fabricated transcript is unrepresentable. External whisper.cpp
//!   arrives with the M15 manifest lane; until then audio-to-text
//!   resolves to the typed `NoProjector` + system notice.
//! - The lossless PCM hash hook (`ffmpeg -i x -map 0:a -f md5 -`) is
//!   the M17 `#[ignore]` tier; the pure gate is exact f32 sample
//!   equality between decode legs (tolerance exists only across
//!   bit-depth conversion, which is quantization shared by both legs).
//! - Thumbnails: none as MediaRef artifacts in v0.2 - the waveform-peak
//!   array in the tree IS the thumbnail data.
//! - A stream that decodes to zero packets (e.g. an mp3 truncated
//!   inside its first frame) fails the typed `ParseFailure` - a
//!   zero-sample extraction would be a fabrication.

use std::collections::BTreeSet;

use harness_core::event::ProjectionKind;
use serde_json::{Value, json};

use crate::descriptor::Operation;
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;
use crate::projection::{
    ProjectionContext, ProjectionProvider, ProjectionRegistry, ProjectionWiringError,
};

pub use super::audio_decode::{DecodedAudio, decode};
pub use super::audio_flac::encode_flac;
pub use super::audio_synth::SynthOutput;
pub use super::audio_wav::{WavDepth, decode_wav, encode_wav};

/// Producer-disclosure engine id for the extract/verify bindings.
pub const ENGINE: &str = "harness-media::audio";

/// Producer-disclosure engine id for the generate binding (the synth
/// writer: hound WAV or flacenc FLAC per the spec's target format).
pub const SYNTH_ENGINE: &str = "harness-media::audio-synth";

/// Waveform-peak thumbnail buckets in the extract tree.
pub const WAVEFORM_PEAK_BUCKETS: usize = 256;

/// The typed opus-decode redirect reason (pinned by tests).
pub const OPUS_DECODE_REDIRECT: &str = "opus decode is unimplemented in symphonia (digest 3.4 status '-'); the pure lane never \
     decodes it - it routes to the ffmpeg subprocess lane (M17)";

/// The typed lossy-encode redirect reason (pinned by tests).
pub const LOSSY_ENCODE_REDIRECT: &str = "the pure tier never claims opus/mp3/vorbis encode; encoding routes to the ffmpeg \
     subprocess lane (M17: libopus/libmp3lame/libvorbis are all GREEN flags there)";

/// Peak + RMS + the waveform-peak thumbnail array over interleaved
/// samples. Bucket boundaries are even fractional slices of the sample
/// range (start = i*len/N), so every bucket is non-empty whenever the
/// stream is at least one bucket long - never a left-packed array with
/// a silent tail.
fn analyze(samples: &[f32]) -> (f64, f64, Vec<f64>) {
    let mut peak = 0_f64;
    let mut energy = 0_f64;
    for sample in samples {
        let magnitude = (*sample as f64).abs();
        peak = peak.max(magnitude);
        energy += magnitude * magnitude;
    }
    let rms = if samples.is_empty() {
        0.0
    } else {
        (energy / samples.len() as f64).sqrt()
    };
    let len = samples.len();
    let mut peaks = Vec::with_capacity(WAVEFORM_PEAK_BUCKETS);
    for bucket in 0..WAVEFORM_PEAK_BUCKETS {
        let start = bucket * len / WAVEFORM_PEAK_BUCKETS;
        let end = (((bucket + 1) * len) / WAVEFORM_PEAK_BUCKETS)
            .max(start + 1)
            .min(len);
        peaks.push(
            samples[start..end]
                .iter()
                .map(|sample| (*sample as f64).abs())
                .fold(0_f64, f64::max),
        );
    }
    (peak, rms, peaks)
}

/// The audio extract handler: decode + analysis to the canonical tree
/// and the bounded one-line text projection
/// (`audio: <codec> <hz>Hz <ch>ch <mm:ss>`).
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let decoded = decode(bytes, budgets)?;
    let (peak, rms, peaks) = analyze(&decoded.samples);
    let frames = decoded.samples.len() / decoded.channels;
    let duration = frames as f64 / f64::from(decoded.sample_rate);
    let tree = json!({
        "channels": decoded.channels,
        "codec": decoded.codec,
        "duration_secs": duration,
        "peak": peak,
        "rms": rms,
        "sample_rate": decoded.sample_rate,
        "samples": frames,
        "waveform_peaks": peaks,
    });
    let minutes = (duration / 60.0) as u64;
    let seconds = (duration - f64::from(60_u32 * minutes as u32)) as u64;
    let projection = format!(
        "audio: {} {}Hz {}ch {minutes}:{seconds:02}",
        decoded.codec, decoded.sample_rate, decoded.channels
    );
    Ok(Extracted {
        tree,
        text_projection: projection,
        thumbnails: Vec::new(),
    })
}

/// The synth facade: validated spec to interleaved samples + layout.
pub fn synth(spec: &GenerationSpec) -> Result<SynthOutput, MediaError> {
    let (parsed, frames) = super::audio_synth::parse_spec(spec)?;
    Ok(super::audio_synth::synth(&parsed, frames))
}

/// The audio generate handler: synth spec to WAV (pcm16, hound) or FLAC
/// (flacenc) bytes per the spec's target format.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let output = synth(spec)?;
    match spec.format {
        FormatId::Wav => encode_wav(
            &output.samples,
            output.channels,
            output.sample_rate,
            WavDepth::Pcm16,
        ),
        FormatId::Flac => encode_flac(&output.samples, output.channels, output.sample_rate),
        _ => Err(MediaError::InvalidSpec),
    }
}

/// The INDEPENDENT spec->tree projection (M5 provenance rule: never the
/// adapter's own extractor over its own output). Projects the clock
/// facts computable from the spec alone - codec (by target container),
/// rate, channels, per-channel frames, duration. Peak/RMS/waveform
/// require the synthesis itself and are NOT part of the comparable
/// form (mirrors the pdf lane's text-subset projection).
pub fn spec_tree(spec: &GenerationSpec) -> Result<Value, MediaError> {
    let (parsed, frames) = super::audio_synth::parse_spec(spec)?;
    Ok(json!({
        "channels": parsed.channels,
        "codec": if spec.format == FormatId::Flac { "flac" } else { "pcm_s16le" },
        "duration_secs": frames as f64 / parsed.sample_rate as f64,
        "sample_rate": parsed.sample_rate,
        "samples": frames,
    }))
}

fn tolerances() -> Tolerances {
    Tolerances {
        float_epsilon: 1e-9,
        unordered_at: BTreeSet::new(),
        ignored_at: BTreeSet::new(),
    }
}

/// The audio verify handler: probe-diff under the waveform tolerance
/// profile - extract the produced bytes, project both sides down to the
/// comparable clock-facts form, structural diff; empty diff = PASS.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let tree = extracted.tree;
    let comparable = json!({
        "channels": tree["channels"],
        "codec": tree["codec"],
        "duration_secs": tree["duration_secs"],
        "sample_rate": tree["sample_rate"],
        "samples": tree["samples"],
    });
    let diff = structural_diff(expected, &comparable, &tolerances())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}

/// The typed refusal for every lossy encode the pure tier never claims.
pub fn lossy_encode_redirect(format: FormatId) -> MediaError {
    MediaError::UnsupportedOperation {
        format,
        op: Operation::Generate,
        reason: LOSSY_ENCODE_REDIRECT.to_owned(),
    }
}

/// The deterministic audio projector (the M10 seam): a thin wrap over
/// the extractor's one-line summary. Kind is TextProjection and ONLY
/// TextProjection - the transcript seam stays model-mediated.
pub struct AudioProjector;

impl ProjectionProvider for AudioProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}

/// Registers [`AudioProjector`] for the family's four pure-decode
/// formats (opus never registers - its decode is the M17 lane).
///
/// # Errors
/// [`ProjectionWiringError::DuplicateProjector`] on contention (one
/// format, one projector).
pub fn register_projections(
    registry: &mut ProjectionRegistry,
) -> Result<(), ProjectionWiringError> {
    for format in [FormatId::Wav, FormatId::Flac, FormatId::Mp3, FormatId::Ogg] {
        registry.register(format, std::sync::Arc::new(AudioProjector))?;
    }
    Ok(())
}
