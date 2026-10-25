//! FLAC encode via flacenc (plan M14: "the only A/V format fully
//! generable in pure Rust" per the digest). The lane is pcm16-in:
//! samples quantize through the shared [`super::audio_wav`] quantizer
//! (so a WAV round trip and a FLAC round trip of the same program
//! compare against the SAME quantization), then flacenc writes a
//! deterministic stream - default encoder config, single-threaded (the
//! `par` feature is off), no clock fields in a FLAC stream, so repeat
//! encodes are byte-identical. FLAC is lossless: decoding the output
//! reproduces the quantized samples exactly (pinned by the round-trip
//! test through the symphonia decode lane).
//!
//! ## STREAMINFO block-size normalization (recorded interop decision)
//!
//! flacenc reports the ACTUAL minimum block size in STREAMINFO (a short
//! final frame shrinks it below the nominal fixed block size), while
//! the fixed-blocksize convention ffmpeg/libFLAC emit - and the
//! symphonia 0.6 reader enforces - is `min == max == nominal` with the
//! final short frame exempt (verified empirically: symphonia decodes
//! ffmpeg's min==max stream and refuses flacenc's min<max one with
//! UnexpectedEof at probe). Post-encode, the stream info is normalized
//! to that convention using flacenc's own `max_block_size` - no frame
//! byte is touched, so the lossless guarantee is intact (decode
//! round-trips the exact quantized samples either way; pinned).

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::config;
use flacenc::error::Verify;
use flacenc::source::MemSource;

use super::audio_wav::quantize_i16;
use crate::error::MediaError;

/// FLAC bits-per-sample of this lane (pcm16 in, pcm16 out).
const BITS_PER_SAMPLE: usize = 16;

/// Encodes interleaved f32 samples to a FLAC stream (pcm16 quantization,
/// lossless from the quantized values).
///
/// # Errors
/// [`MediaError::InvalidSpec`] if flacenc refuses the configuration or
/// the stream (unreachable for in-range pcm16 input; the pdf lane's
/// write-error mapping precedent).
pub fn encode_flac(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
) -> Result<Vec<u8>, MediaError> {
    let interleaved: Vec<i32> = samples
        .iter()
        .map(|sample| i32::from(quantize_i16(*sample)))
        .collect();
    let source = MemSource::from_samples(
        &interleaved,
        usize::from(channels),
        BITS_PER_SAMPLE,
        sample_rate as usize,
    );
    let verified = config::Encoder::default()
        .into_verified()
        .map_err(|_| MediaError::InvalidSpec)?;
    let mut stream = flacenc::encode_with_fixed_block_size(&verified, source, verified.block_size)
        .map_err(|_| MediaError::InvalidSpec)?;
    let nominal = stream.stream_info().max_block_size();
    stream
        .stream_info_mut()
        .set_block_sizes(nominal, nominal)
        .map_err(|_| MediaError::InvalidSpec)?;
    let mut sink = ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|_| MediaError::InvalidSpec)?;
    Ok(sink.into_inner())
}
