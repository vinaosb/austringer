//! WAV transcode I/O via hound (plan M14: "WAV read/write via hound -
//! decode to f32; write pcm16/32/f32"). The pcm16 quantizer is the
//! single shared definition (saturating `as i16` cast after a
//! 32_768-scale round), so the synth writer, the flac encoder, and the
//! bit-exact round-trip tests all quantize identically: `i16/32768` is
//! exactly representable in f32, which is what makes the lossless
//! round trip EXACT sample equality instead of a tolerance.

use std::io::Cursor;

use hound::{SampleFormat, WavSpec, WavWriter};

use super::audio_decode::DecodedAudio;
use crate::error::MediaError;

/// The output depth of the WAV writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavDepth {
    /// Signed 16-bit PCM (the synth writer's fixed depth).
    Pcm16,
    /// Signed 32-bit PCM.
    Pcm32,
    /// IEEE 754 32-bit float (no quantization at all).
    F32,
}

/// The one pcm16 quantizer (saturating cast: +1.0 maps to +32767).
pub(super) fn quantize_i16(sample: f32) -> i16 {
    ((sample.clamp(-1.0, 1.0) * 32_768.0).round()) as i16
}

/// Writes interleaved f32 samples as a WAV file of the requested depth.
///
/// # Errors
/// [`MediaError::InvalidSpec`] if the writer refuses (non-representable
/// layout); hound writes into memory, so I/O failures are not a lane
/// here.
pub fn encode_wav(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    depth: WavDepth,
) -> Result<Vec<u8>, MediaError> {
    let (bits, format) = match depth {
        WavDepth::Pcm16 | WavDepth::Pcm32 => (depth.bits_per_sample(), SampleFormat::Int),
        WavDepth::F32 => (depth.bits_per_sample(), SampleFormat::Float),
    };
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: bits,
        sample_format: format,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec).map_err(|_| MediaError::InvalidSpec)?;
        for sample in samples {
            match depth {
                WavDepth::Pcm16 => {
                    writer
                        .write_sample(quantize_i16(*sample))
                        .map_err(|_| MediaError::InvalidSpec)?;
                }
                WavDepth::Pcm32 => {
                    let value = (sample.clamp(-1.0, 1.0) * 2_147_483_648.0).round();
                    writer
                        .write_sample(value.clamp(-2_147_483_648.0, 2_147_483_647.0) as i32)
                        .map_err(|_| MediaError::InvalidSpec)?;
                }
                WavDepth::F32 => {
                    writer
                        .write_sample(*sample)
                        .map_err(|_| MediaError::InvalidSpec)?;
                }
            }
        }
        writer.finalize().map_err(|_| MediaError::InvalidSpec)?;
    }
    Ok(cursor.into_inner())
}

/// Reads a WAV file to interleaved f32 at native scale (integer depths
/// divide by their own full scale: 2^15 / 2^23 / 2^31; float depths are
/// exact).
///
/// # Errors
/// [`MediaError::ParseFailure`] for any stream hound refuses.
pub fn decode_wav(bytes: &[u8]) -> Result<DecodedAudio, MediaError> {
    let mut reader =
        hound::WavReader::new(Cursor::new(bytes)).map_err(|_| MediaError::ParseFailure)?;
    let spec = reader.spec();
    let codec = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => "pcm_s16le",
        (SampleFormat::Int, 24) => "pcm_s24le",
        (SampleFormat::Int, 32) => "pcm_s32le",
        (SampleFormat::Float, 32) => "pcm_f32le",
        _ => return Err(MediaError::ParseFailure),
    };
    let scale = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => 32_768.0,
        (SampleFormat::Int, 24) => 8_388_608.0,
        (SampleFormat::Int, 32) => 2_147_483_648.0,
        _ => 1.0,
    };
    let samples: Vec<f32> = if spec.sample_format == SampleFormat::Float {
        reader
            .samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|_| MediaError::ParseFailure)?
    } else {
        reader
            .samples::<i32>()
            .map(|sample| sample.map(|value| value as f32 / scale as f32))
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|_| MediaError::ParseFailure)?
    };
    Ok(DecodedAudio {
        samples,
        channels: spec.channels as usize,
        sample_rate: spec.sample_rate,
        codec: codec.to_owned(),
    })
}

impl WavDepth {
    fn bits_per_sample(self) -> u16 {
        match self {
            Self::Pcm16 => 16,
            Self::Pcm32 | Self::F32 => 32,
        }
    }
}
