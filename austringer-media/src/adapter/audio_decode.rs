//! audio decode: the symphonia lane (plan M14). One bounded loop over
//! every packet of the default audio track, converting each decoded
//! buffer to interleaved f32 at native scale (symphonia 0.6 FLAC shifts
//! sub-32-bit streams to full i32 scale before the variant conversion,
//! so the integer normalization is width-exact; the corpus peak pin
//! ~0.125 is the standing scale guard).
//!
//! Budget enforcement is per packet (wall-clock deadline) plus a
//! cumulative materialization cap (input bytes + 4 bytes per f32 sample
//! appended); tripping either is the typed `ExtractTooLarge`. Codec
//! errors map to the typed `ParseFailure`; recoverable `DecodeError`
//! packets are skipped (a stream whose packets ALL fail decodes to
//! nothing and is refused whole). OPUS is refused BEFORE any decoder is
//! instantiated: symphonia has no opus decoder (digest 3.4 status "-"),
//! so the pure lane redirects decode to the M17 ffmpeg subprocess lane
//! with the typed reason - never a partial or faked decode.

use std::io::Cursor;
use std::time::Instant;

use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::well_known::{
    CODEC_ID_FLAC, CODEC_ID_MP3, CODEC_ID_OPUS, CODEC_ID_PCM_F32LE, CODEC_ID_PCM_S16LE,
    CODEC_ID_PCM_S24LE, CODEC_ID_PCM_S32LE, CODEC_ID_VORBIS,
};
use symphonia::core::codecs::audio::{AudioCodecId, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};

use crate::descriptor::Operation;
use crate::error::MediaError;
use crate::extract::ExtractBudgets;
use crate::format::FormatId;

/// Fully decoded audio: interleaved f32 samples in [-1, 1] (lossy codecs
/// may momentarily exceed it), channel count, sample rate, and the
/// canonical codec name.
pub struct DecodedAudio {
    /// Interleaved samples (frame-major, channel-minor).
    pub samples: Vec<f32>,
    /// Channel count.
    pub channels: usize,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Canonical codec name (`pcm_s16le`, `flac`, `mp3`, `vorbis`, ...).
    pub codec: String,
}

/// The typed opus redirect, shared by every entry point that could be
/// handed opus bytes (dispatch already routes `FormatId::Opus` to the
/// subprocess lane; this guards the decode seam itself).
pub(super) fn opus_redirect() -> MediaError {
    MediaError::UnsupportedOperation {
        format: FormatId::Opus,
        op: Operation::Extract,
        reason: crate::adapter::audio::OPUS_DECODE_REDIRECT.to_owned(),
    }
}

/// Decode any pure-lane audio format to f32 under the caller's budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] for undecodable streams (including a
/// stream that yields no decodable packet), [`MediaError::ExtractTooLarge`]
/// when a budget trips, and the typed opus redirect for opus payloads.
pub fn decode(bytes: &[u8], budgets: ExtractBudgets) -> Result<DecodedAudio, MediaError> {
    // The opus guard runs BEFORE symphonia touches the bytes: our own M2
    // sniff identifies opus payloads (OggS + OpusHead) that the pure lane
    // must never decode.
    if let crate::probe::ProbeResult::Identified {
        format: FormatId::Opus,
        ..
    } = crate::probe::probe(bytes, None)
    {
        return Err(opus_redirect());
    }
    let start = Instant::now();
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut reader = get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|_| MediaError::ParseFailure)?;
    let track = reader
        .default_track(TrackType::Audio)
        .ok_or(MediaError::ParseFailure)?;
    let params = track
        .codec_params
        .as_ref()
        .ok_or(MediaError::ParseFailure)?;
    let CodecParameters::Audio(audio) = params else {
        return Err(MediaError::ParseFailure);
    };
    // Second opus guard for streams whose OpusHead hides beyond the M2
    // sniff window: the codec parameters name the codec symphonia has no
    // decoder for.
    if audio.codec == CODEC_ID_OPUS {
        return Err(opus_redirect());
    }
    let codec = codec_name(audio.codec);
    let track_id = track.id;
    let mut decoder = get_codecs()
        .make_audio_decoder(audio, &AudioDecoderOptions::default())
        .map_err(|_| MediaError::ParseFailure)?;

    // copy_to_vec_interleaved has per-packet RESIZE-and-overwrite
    // semantics, so each packet lands in scratch and is appended here.
    let mut samples: Vec<f32> = Vec::new();
    let mut packet_samples: Vec<f32> = Vec::new();
    let mut spent = bytes.len() as u64;
    let mut layout: Option<(u32, usize)> = None;
    loop {
        if start.elapsed() > budgets.deadline {
            return Err(MediaError::ExtractTooLarge);
        }
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::IoError(io)) if io.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(_) => return Err(MediaError::ParseFailure),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => return Err(MediaError::ParseFailure),
        };
        if layout.is_none() {
            let spec = decoded.spec();
            layout = Some((spec.rate(), spec.channels().count()));
        }
        decoded.copy_to_vec_interleaved(&mut packet_samples);
        samples.extend_from_slice(&packet_samples);
        spent = spent.saturating_add(u64::try_from(samples.len()).unwrap_or(u64::MAX) * 4);
        if spent > budgets.max_total_bytes {
            return Err(MediaError::ExtractTooLarge);
        }
    }
    let Some((sample_rate, channels)) = layout else {
        return Err(MediaError::ParseFailure);
    };
    if channels == 0 || samples.is_empty() || !samples.len().is_multiple_of(channels) {
        return Err(MediaError::ParseFailure);
    }
    Ok(DecodedAudio {
        samples,
        channels,
        sample_rate,
        codec,
    })
}

/// Canonical codec names for the family's four pure lanes (anything
/// outside the family reports `unknown` - it never reaches this adapter
/// through dispatch).
fn codec_name(codec: AudioCodecId) -> String {
    let name = match codec {
        CODEC_ID_PCM_S16LE => "pcm_s16le",
        CODEC_ID_PCM_S24LE => "pcm_s24le",
        CODEC_ID_PCM_S32LE => "pcm_s32le",
        CODEC_ID_PCM_F32LE => "pcm_f32le",
        CODEC_ID_FLAC => "flac",
        CODEC_ID_MP3 => "mp3",
        CODEC_ID_VORBIS => "vorbis",
        _ => "unknown",
    };
    name.to_owned()
}
