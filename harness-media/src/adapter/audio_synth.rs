//! audio synth: the spec-driven tone generator (plan M14).
//!
//! Spec contract (typed, validated before any allocation):
//! `tones: [{freq, start, dur, gain, wave: sine|triangle|square|noise}]`,
//! `sample_rate`, `channels`. Programs are mixed by summation and clamped
//! to [-1, 1]; every channel carries the same mono program (there is no
//! pan field - an honest subset, not a fake stereo stage).
//!
//! DETERMINISM CONTRACT: the noise wave is a fixed-seed xorshift\* per
//! tone (seed derived from the tone index and a pinned constant), so the
//! same spec synthesizes byte-identical audio on every run - ambient
//! entropy never enters the generator (pinned by the repeat-generation
//! test).

use serde::Deserialize;

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Tone-count cap (pathological specs fail typed InvalidSpec).
const MAX_TONES: usize = 64;
/// Sample-rate bounds (Hz).
const MIN_RATE: u64 = 8_000;
const MAX_RATE: u64 = 192_000;
/// Channel bounds (FLAC and hound both cap at 8).
const MAX_CHANNELS: u64 = 8;
/// Frequency bound (Hz; Nyquist-safe upper limit, not just < rate/2 -
/// honest tone synthesis above 96 kHz has no consumer here).
const MAX_FREQ: f64 = 96_000.0;
/// Total interleaved-sample cap (64 MiB of f32) and per-frame tone-work
/// cap: both trip the typed InvalidSpec instead of the allocator.
const MAX_TOTAL_SAMPLES: u64 = 16_000_000;
const MAX_FRAME_TONE_WORK: u64 = 64_000_000;
/// The pinned xorshift\* seed offset (any fixed constant keeps the
/// generator deterministic; this one is the golden-ratio fractional).
const NOISE_SEED_OFFSET: u64 = 0x9E37_79B9_7F4A_7C15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Waveform {
    Sine,
    Triangle,
    Square,
    Noise,
}

#[derive(Debug, Clone, Deserialize)]
struct ToneSpec {
    freq: f64,
    start: f64,
    dur: f64,
    gain: f64,
    wave: Waveform,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct SynthSpec {
    tones: Vec<ToneSpec>,
    pub(super) sample_rate: u64,
    pub(super) channels: u64,
}

/// The synthesized program: interleaved f32 samples + layout.
pub struct SynthOutput {
    /// Interleaved samples in [-1, 1].
    pub samples: Vec<f32>,
    /// Channel count.
    pub channels: u16,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Total per-channel frame count.
    pub frames: u64,
}

fn invalid() -> MediaError {
    MediaError::InvalidSpec
}

fn finite_in_range(value: f64, low: f64, high: f64) -> bool {
    value.is_finite() && value >= low && value <= high
}

/// Parses + validates the synth spec (shared by the writer, the synth
/// facade, and the independent spec->tree projection, so one contract
/// governs all three).
pub(super) fn parse_spec(spec: &GenerationSpec) -> Result<(SynthSpec, u64), MediaError> {
    if !matches!(spec.format, FormatId::Wav | FormatId::Flac) {
        return Err(invalid());
    }
    let parsed: SynthSpec = serde_json::from_value(spec.params.clone()).map_err(|_| invalid())?;
    if parsed.tones.is_empty() || parsed.tones.len() > MAX_TONES {
        return Err(invalid());
    }
    if !(MIN_RATE..=MAX_RATE).contains(&parsed.sample_rate)
        || parsed.channels == 0
        || parsed.channels > MAX_CHANNELS
    {
        return Err(invalid());
    }
    let mut span = 0_f64;
    for tone in &parsed.tones {
        if !finite_in_range(tone.freq, 0.0, MAX_FREQ)
            || !finite_in_range(tone.gain, 0.0, 1.0)
            || !finite_in_range(tone.start, 0.0, 3_600.0)
            || !finite_in_range(tone.dur, f64::MIN_POSITIVE, 3_600.0)
        {
            return Err(invalid());
        }
        span = span.max(tone.start + tone.dur);
    }
    if !span.is_finite() {
        return Err(invalid());
    }
    let frames = (span * parsed.sample_rate as f64).ceil();
    let total = frames * parsed.channels as f64;
    if !frames.is_finite() || !total.is_finite() || total > MAX_TOTAL_SAMPLES as f64 {
        return Err(invalid());
    }
    if frames * parsed.tones.len() as f64 > MAX_FRAME_TONE_WORK as f64 {
        return Err(invalid());
    }
    Ok((parsed, frames as u64))
}

struct Noise(u64);

impl Noise {
    fn new(tone_index: usize) -> Self {
        Self(NOISE_SEED_OFFSET ^ ((tone_index as u64).wrapping_mul(0x2545_F491_4F6C_DD1D)) | 1)
    }

    fn next_f64(&mut self) -> f64 {
        let Self(state) = self;
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state as f64 / u64::MAX as f64) * 2.0 - 1.0
    }
}

fn wave_sample(wave: Waveform, freq: f64, t: f64) -> f64 {
    let phase = (t * freq) % 1.0;
    match wave {
        Waveform::Sine => (2.0 * std::f64::consts::PI * phase).sin(),
        // Piecewise triangle over the fractional phase: peak at 0.25,
        // zero crossings at 0 and 0.5, symmetric trough at 0.75.
        Waveform::Triangle => 4.0 * (phase - 0.25).abs() - 1.0,
        Waveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        // The noise branch never consults the phase: its stream is the
        // per-tone xorshift sequence (see `synth`).
        Waveform::Noise => 0.0,
    }
}

/// Synthesizes the validated spec. Pure function of the spec - no
/// ambient state, no clock, no allocation beyond the sample buffer.
pub(super) fn synth(parsed: &SynthSpec, frames: u64) -> SynthOutput {
    let rate = parsed.sample_rate;
    let channels = parsed.channels as usize;
    let mut noise_streams: Vec<Option<Noise>> =
        parsed.tones.iter().enumerate().map(|(index, tone)| {
            (tone.wave == Waveform::Noise).then(|| Noise::new(index))
        }).collect();
    let mut samples = Vec::with_capacity((frames as usize).saturating_mul(channels));
    for frame in 0..frames {
        let t = frame as f64 / rate as f64;
        let mut mixed = 0_f64;
        for (tone, noise) in parsed.tones.iter().zip(&mut noise_streams) {
            let local = t - tone.start;
            if local < 0.0 || local >= tone.dur {
                continue;
            }
            let amplitude = if let Some(stream) = noise.as_mut() {
                stream.next_f64()
            } else {
                wave_sample(tone.wave, tone.freq, local)
            };
            mixed += tone.gain * amplitude;
        }
        let clamped = mixed.clamp(-1.0, 1.0) as f32;
        for _ in 0..channels {
            samples.push(clamped);
        }
    }
    SynthOutput { samples, channels: channels as u16, sample_rate: rate as u32, frames }
}
