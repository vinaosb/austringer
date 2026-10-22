//! M14 audio adapter round-trip suite: synth->probe, synth->extract
//! against the INDEPENDENT spec->tree projection (M5 provenance rule),
//! the transcode seams (hound WAV read/write pcm16/32/f32, flacenc FLAC
//! encode), lossless bit-exactness (decode both sides to f32, exact
//! sample equality; tolerance exists only across bit-depth conversion),
//! synth determinism (noise included), and the typed lossy-encode
//! redirect to the M17 ffmpeg lane. Hermetic.

use harness_media::adapter::audio::{self, WavDepth};
use harness_media::error::MediaError;
use harness_media::executor::OpRequest;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::json;

fn two_tone_spec(format: FormatId) -> GenerationSpec {
    GenerationSpec {
        format,
        params: json!({
            "tones": [
                {"freq": 440.0, "start": 0.0, "dur": 0.25, "gain": 0.5, "wave": "sine"},
                {"freq": 880.0, "start": 0.1, "dur": 0.15, "gain": 0.25, "wave": "triangle"}
            ],
            "sample_rate": 16_000,
            "channels": 2
        }),
    }
}

/// The tone span (0.25 s) as frames at 16 kHz, per channel.
const FRAMES: u64 = 4_000;

#[test]
fn synth_wav_probes_and_round_trips_the_independent_tree() {
    // Given: the two-tone stereo synth spec, wav target.
    let spec = two_tone_spec(FormatId::Wav);
    // When: generated.
    let bytes = audio::generate(&spec).expect("synth must generate");
    // Then: the bytes probe back as wav (Certain)...
    match harness_media::probe::probe(&bytes, None) {
        harness_media::probe::ProbeResult::Identified { format, .. } => {
            assert_eq!(format, FormatId::Wav);
        }
        other => panic!("synth output must probe Identified, got {other:?}"),
    }
    // ...and verify against the INDEPENDENT spec->tree projection passes
    // (empty structural diff - never the extractor over its own output).
    let expected = audio::spec_tree(&spec).expect("spec tree");
    assert_eq!(
        expected,
        json!({
            "channels": 2,
            "codec": "pcm_s16le",
            "duration_secs": 0.25,
            "sample_rate": 16_000,
            "samples": FRAMES
        })
    );
    let verdict = audio::verify(&expected, &bytes).expect("verify must run");
    assert!(verdict.is_pass(), "round trip must pass: {verdict:?}");
    // And: the same verdict through the executor dispatch, with the synth
    // producer disclosure.
    let produced = harness_media::executor::registry()
        .dispatch(
            FormatId::Wav,
            OpRequest::Verify {
                expected: &expected,
                bytes: &bytes,
            },
        )
        .expect("verify dispatches");
    assert_eq!(produced.producer().engine, audio::ENGINE);
    assert!(
        matches!(produced.inner(), harness_media::executor::OpOutput::Verify(v) if v.is_pass())
    );
}

#[test]
fn synth_flac_probes_and_round_trips_the_independent_tree() {
    // Given: the same spec aimed at flac (flacenc is the only A/V
    // encoder the pure tier honestly owns).
    let spec = two_tone_spec(FormatId::Flac);
    let bytes = audio::generate(&spec).expect("flac synth must generate");
    // Then: probes as flac, verifies against the independent tree.
    match harness_media::probe::probe(&bytes, None) {
        harness_media::probe::ProbeResult::Identified { format, .. } => {
            assert_eq!(format, FormatId::Flac);
        }
        other => panic!("flac synth must probe Identified, got {other:?}"),
    }
    let expected = audio::spec_tree(&spec).expect("spec tree");
    assert_eq!(expected["codec"], json!("flac"));
    let verdict = audio::verify(&expected, &bytes).expect("verify must run");
    assert!(verdict.is_pass(), "flac round trip must pass: {verdict:?}");
}

#[test]
fn synth_is_byte_deterministic_noise_included() {
    // Given: a noise-wave spec (the noise generator must be seeded from
    // constants - never ambient entropy).
    let spec = GenerationSpec {
        format: FormatId::Wav,
        params: json!({
            "tones": [
                {"freq": 1000.0, "start": 0.0, "dur": 0.1, "gain": 0.3, "wave": "noise"},
                {"freq": 500.0, "start": 0.0, "dur": 0.1, "gain": 0.2, "wave": "square"}
            ],
            "sample_rate": 8_000,
            "channels": 1
        }),
    };
    // When: generated twice.
    let first = audio::generate(&spec).expect("first synth");
    let second = audio::generate(&spec).expect("second synth");
    // Then: byte-identical (the M1 determinism contract).
    assert_eq!(first, second);
}

#[test]
fn invalid_synth_specs_fail_the_typed_invalid_spec() {
    // Given: specs outside the adapter contract (no tones, zero rate,
    // out-of-range gain, over-length program).
    let cases: [GenerationSpec; 4] = [
        GenerationSpec {
            format: FormatId::Wav,
            params: json!({"tones": [], "sample_rate": 16_000, "channels": 1}),
        },
        GenerationSpec {
            format: FormatId::Wav,
            params: json!({"tones": [{"freq": 440.0, "start": 0.0, "dur": 0.1, "gain": 0.5, "wave": "sine"}], "sample_rate": 0, "channels": 1}),
        },
        GenerationSpec {
            format: FormatId::Wav,
            params: json!({"tones": [{"freq": 440.0, "start": 0.0, "dur": 0.1, "gain": 2.0, "wave": "sine"}], "sample_rate": 16_000, "channels": 1}),
        },
        GenerationSpec {
            format: FormatId::Wav,
            params: json!({"tones": [{"freq": 440.0, "start": 0.0, "dur": 3_600.0, "gain": 0.5, "wave": "sine"}], "sample_rate": 16_000, "channels": 1}),
        },
    ];
    for spec in cases {
        // When: generated. Then: the typed InvalidSpec, never a panic,
        // never an unbounded allocation.
        let error = audio::generate(&spec).expect_err("spec must be refused");
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
    // And: a spec naming a format outside this adapter's generate lane is
    // refused the same way.
    let wrong = GenerationSpec {
        format: FormatId::Opus,
        params: json!({"tones": [{"freq": 440.0, "start": 0.0, "dur": 0.1, "gain": 0.5, "wave": "sine"}], "sample_rate": 16_000, "channels": 1}),
    };
    assert!(matches!(
        audio::generate(&wrong),
        Err(MediaError::InvalidSpec)
    ));
}

#[test]
fn wav_pcm16_transcode_is_bit_exact_and_byte_deterministic() {
    // Given: synth samples and their pcm16 quantization.
    let spec = GenerationSpec {
        format: FormatId::Wav,
        params: json!({
            "tones": [{"freq": 440.0, "start": 0.0, "dur": 0.2, "gain": 0.9, "wave": "sine"}],
            "sample_rate": 16_000,
            "channels": 2
        }),
    };
    let synthesized = audio::synth(&spec).expect("synth");
    let quantized: Vec<f32> = synthesized
        .samples
        .iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * 32_768.0).round() as i16 as f32 / 32_768.0)
        .collect();
    // When: written as pcm16 wav and read back via hound.
    let wav = audio::encode_wav(
        &synthesized.samples,
        synthesized.channels as u16,
        synthesized.sample_rate,
        WavDepth::Pcm16,
    )
    .expect("wav encode");
    let decoded = audio::decode_wav(&wav).expect("wav decode");
    // Then: EXACT sample equality with the quantization (tolerance is
    // owed only across bit-depth conversion, and this is lossless within
    // the depth).
    assert_eq!(
        decoded.samples, quantized,
        "pcm16 wav round trip is bit-exact"
    );
    assert_eq!(decoded.sample_rate, 16_000);
    assert_eq!(decoded.channels, 2);
    // And: re-encoding the decoded samples reproduces the same bytes
    // (byte-determinism of the writer).
    let reencoded = audio::encode_wav(
        &decoded.samples,
        decoded.channels as u16,
        decoded.sample_rate,
        WavDepth::Pcm16,
    )
    .expect("wav re-encode");
    assert_eq!(wav, reencoded, "repeat encode is byte-identical");
}

#[test]
fn wav_f32_transcode_is_bit_exact() {
    // Given: synth samples written as ieee-float wav (no quantization
    // step at all).
    let spec = GenerationSpec {
        format: FormatId::Wav,
        params: json!({
            "tones": [{"freq": 300.0, "start": 0.0, "dur": 0.1, "gain": 0.7, "wave": "square"}],
            "sample_rate": 8_000,
            "channels": 1
        }),
    };
    let synthesized = audio::synth(&spec).expect("synth");
    let wav = audio::encode_wav(
        &synthesized.samples,
        synthesized.channels as u16,
        synthesized.sample_rate,
        WavDepth::F32,
    )
    .expect("f32 wav encode");
    let decoded = audio::decode_wav(&wav).expect("f32 wav decode");
    // Then: exact f32 equality - the float lane never quantizes.
    assert_eq!(decoded.samples, synthesized.samples);
}

#[test]
fn flac_encode_round_trips_losslessly_through_symphonia() {
    // Given: synth samples, their pcm16 quantization, and a flacenc
    // encode of them (the pure tier's only A/V encode).
    let spec = GenerationSpec {
        format: FormatId::Flac,
        params: json!({
            "tones": [{"freq": 440.0, "start": 0.0, "dur": 0.2, "gain": 0.6, "wave": "triangle"}],
            "sample_rate": 16_000,
            "channels": 2
        }),
    };
    let synthesized = audio::synth(&spec).expect("synth");
    let quantized: Vec<f32> = synthesized
        .samples
        .iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * 32_768.0).round() as i16 as f32 / 32_768.0)
        .collect();
    let flac = audio::encode_flac(
        &synthesized.samples,
        synthesized.channels as u16,
        synthesized.sample_rate,
    )
    .expect("flac encode");
    // When: decoded through the symphonia decode lane.
    let decoded = audio::decode(&flac, harness_media::extract::ExtractBudgets::default())
        .expect("flac decode");
    // Then: FLAC is lossless - exact sample equality with the pcm16
    // quantization, and the full metadata survives.
    assert_eq!(decoded.samples, quantized, "flac round trip is lossless");
    assert_eq!(decoded.codec, "flac");
    assert_eq!(decoded.sample_rate, 16_000);
    assert_eq!(decoded.channels, 2);
    // And: the same generated spec, through the executor's generate
    // dispatch, produces the identical bytes (determinism + disclosure).
    let produced = harness_media::executor::registry()
        .dispatch(FormatId::Flac, OpRequest::Generate { spec: &spec })
        .expect("flac generate dispatches");
    assert_eq!(produced.producer().engine, audio::SYNTH_ENGINE);
    assert!(
        matches!(produced.inner(), harness_media::executor::OpOutput::Generate(bytes) if bytes == &flac)
    );
}

#[test]
fn lossy_encode_is_typed_redirect_to_the_m17_ffmpeg_lane() {
    // Given: the three lossy codecs the pure tier never claims to encode
    // (libopus/libmp3lame/libvorbis are all GREEN flags - they route to
    // ffmpeg, no fake pure-Rust encoder exists).
    for format in [FormatId::Opus, FormatId::Mp3, FormatId::Ogg] {
        // When: the redirect is asked for.
        let error = audio::lossy_encode_redirect(format);
        // Then: typed UnsupportedOperation naming ffmpeg and M17.
        match error {
            MediaError::UnsupportedOperation {
                format: refused,
                op,
                reason,
            } => {
                assert_eq!(refused, format);
                assert_eq!(op, harness_media::descriptor::Operation::Generate);
                assert!(
                    reason.contains("ffmpeg") && reason.contains("M17"),
                    "reason names the subprocess lane: {reason}"
                );
            }
            other => panic!("{format}: expected UnsupportedOperation, got {other:?}"),
        }
    }
    // And: the executor pins the same honesty at dispatch (mp3/ogg
    // generate refuse with the encode-redirect reason).
    for format in [FormatId::Mp3, FormatId::Ogg] {
        let spec = GenerationSpec {
            format,
            params: json!({}),
        };
        let error = harness_media::executor::registry()
            .dispatch(format, OpRequest::Generate { spec: &spec })
            .expect_err("lossy generate must refuse");
        match error {
            MediaError::UnsupportedOperation { reason, .. } => assert!(
                reason.contains("ffmpeg"),
                "dispatch reason names ffmpeg: {reason}"
            ),
            other => panic!("{format}: expected UnsupportedOperation, got {other:?}"),
        }
    }
}
