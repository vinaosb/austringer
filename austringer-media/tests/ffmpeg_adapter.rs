//! M17 ffmpeg adapter tests: the hermetic mocktool lane (real captured
//! goldens; zero real tools, zero network). Pins the codec-clean
//! transcode policy (RED input/output line, constraint 7), the
//! ffprobe census tree, the fixed argv template set (filter-injection
//! guard), staging hygiene, and the probe-diff verify with the lossy
//! duration tolerance max(1 frame, 1%).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use austringer_core::event::MediaStore;
use austringer_media::adapter::ffmpeg::{
    self, Caps, FFMPEG_TOOL, FFPROBE_TOOL, Target, TranscodeSpec, Trim,
};
use austringer_media::error::MediaError;
use austringer_media::format::FormatId;
use austringer_media::tool::{ToolConfig, ToolExecutor, ToolOverride};

/// Compile-time-anchored golden dirs (hardcoded `"\\..\\..."` backslash
/// suffixes break on unix, where the backslash is not a path separator).
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("austringer-test-support")
        .join("golden")
        .join("mocktool")
}

fn input_dir() -> PathBuf {
    golden_dir().join("_input")
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(input_dir().join(name)).unwrap_or_default()
}

/// T40-style re-exec child entries: one per TOOL so a single adapter
/// call that spawns ffmpeg AND ffprobe is drivable hermetically (the
/// mode words arrive through distinct env vars, not one shared
/// austringer_MOCKTOOL_ARGV).
#[test]
fn mocktool_ffmpeg_child() {
    if let Ok(words) = std::env::var("austringer_MOCKTOOL_FFMPEG_ARGV") {
        std::process::exit(austringer_test_support::mocktool::dispatch_words(&words));
    }
}

#[test]
fn mocktool_ffprobe_child() {
    if let Ok(words) = std::env::var("austringer_MOCKTOOL_FFPROBE_ARGV") {
        std::process::exit(austringer_test_support::mocktool::dispatch_words(&words));
    }
}

fn child_entry(test_name: &str) -> ToolOverride {
    ToolOverride {
        program: Some(
            std::env::current_exe()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        args: vec!["--exact".into(), test_name.into(), "--nocapture".into()],
        timeout_ms: None,
    }
}

struct Lane {
    tools: ToolExecutor,
    env: BTreeMap<String, String>,
}

fn lane(ffmpeg_mode: &str, ffprobe_mode: &str) -> Lane {
    let temp = tempfile::tempdir().expect("tempdir");
    let session_temp = temp.path().to_path_buf();
    std::mem::forget(temp); // outlives the lane; staged files are cleaned per-op
    let tools = ToolExecutor::new(ToolConfig {
        enabled: [
            (FFMPEG_TOOL.to_owned(), child_entry("mocktool_ffmpeg_child")),
            (
                FFPROBE_TOOL.to_owned(),
                child_entry("mocktool_ffprobe_child"),
            ),
        ]
        .into(),
        install_enabled: false,
        session_temp: session_temp.clone(),
        env: BTreeMap::new(),
    });
    let mut env = BTreeMap::new();
    env.insert(
        "austringer_MOCKTOOL_FFMPEG_ARGV".to_owned(),
        ffmpeg_mode.to_owned(),
    );
    env.insert(
        "austringer_MOCKTOOL_FFPROBE_ARGV".to_owned(),
        ffprobe_mode.to_owned(),
    );
    env.insert(
        "austringer_MOCKTOOL_GOLDEN_DIR".to_owned(),
        golden_dir().display().to_string(),
    );
    Lane { tools, env }
}

impl Lane {
    fn adapter(&self) -> ffmpeg::Adapter<'_> {
        ffmpeg::Adapter::new(&self.tools).env(&self.env)
    }
}

/// PROBE: the real ffprobe golden parses to the pinned census tree
/// (wav: pcm_s16le 16 kHz mono 0.2 s; webm: vp9 64x48 @10 fps + opus).
#[test]
fn mock_probe_parses_to_pinned_tree() {
    let av = lane("sleep 0", "golden ffprobe/probe-av-json");
    let probe = av
        .adapter()
        .probe_av(&fixture("tiny.wav"))
        .expect("wav census");
    assert_eq!(probe.tree()["format"], "wav");
    assert_eq!(probe.tree()["duration_secs"], 0.2);
    assert_eq!(probe.tree()["streams"][0]["codec"], "pcm_s16le");
    assert_eq!(probe.tree()["streams"][0]["sample_rate"], 16_000);
    assert_eq!(probe.tree()["streams"][0]["channels"], 1);
    assert!(!probe.has_video());
    assert!(probe.red_codec().is_none());
    assert!(probe.text_projection().starts_with("av: wav 1 stream"));

    let av = lane("sleep 0", "golden ffprobe/probe-webm-json");
    let probe = av
        .adapter()
        .probe_av(&fixture("tiny-vp9.webm"))
        .expect("webm census");
    assert_eq!(probe.tree()["format"], "matroska,webm");
    assert_eq!(probe.tree()["streams"].as_array().map(Vec::len), Some(2));
    assert_eq!(probe.tree()["streams"][0]["codec"], "vp9");
    assert_eq!(probe.tree()["streams"][0]["width"], 64);
    assert_eq!(probe.tree()["streams"][0]["height"], 48);
    assert_eq!(probe.tree()["streams"][0]["fps"], 10.0);
    assert_eq!(probe.tree()["streams"][1]["codec"], "opus");
    assert_eq!(probe.tree()["streams"][1]["sample_rate"], 48_000);
    assert!(probe.has_video());
    assert_eq!(probe.video_width(), Some(64));
}

/// TRANSCODE: spec -> mock pass-through -> probe-diff PASS, with the
/// producer disclosure carrying the clean codec flags verbatim.
#[test]
fn transcode_spec_mock_passthrough_probe_diff_pass() {
    let av = lane("stagecopy", "golden ffprobe/probe-webm-json");
    let input = fixture("tiny-vp9.webm");
    let adapter = av.adapter();
    let census = adapter.probe_av(&input).expect("before census");
    let spec = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps::default(),
        trim: None,
    };
    let artifact = adapter
        .transcode(&input, &census, &spec)
        .expect("clean transcode runs");
    assert_eq!(artifact.bytes, input, "mock pass-through echoes the input");
    assert_eq!(artifact.producer.engine, FFMPEG_TOOL);
    assert_eq!(
        artifact.producer.tier,
        austringer_media::executor::AdapterTier::Subprocess
    );
    // The mock lane's command line is the substitution triple, so the
    // codec-flag disclosure is pinned by target_argv (this suite) and
    // the real-tool tier, not by the mock run.

    let after = adapter.probe_av(&artifact.bytes).expect("after census");
    assert_eq!(
        ffmpeg::verify_transcode(&census, &after, &spec).expect("probe-diff runs"),
        austringer_media::diff::RoundTripVerdict::Pass,
        "probe-diff on the declared target passes"
    );
}

/// REMUX (clean input): stream-copy of GREEN payloads runs; the mock
/// pass-through echoes the bytes and the producer shows `-c copy`.
#[test]
fn remux_clean_input_runs_with_copy_flags() {
    let av = lane("stagecopy", "golden ffprobe/probe-webm-json");
    let input = fixture("tiny-vp9.webm");
    let adapter = av.adapter();
    let census = adapter.probe_av(&input).expect("census");
    let spec = TranscodeSpec {
        target: Target::RemuxWebm,
        caps: Caps::default(),
        trim: None,
    };
    let artifact = adapter
        .transcode(&input, &census, &spec)
        .expect("clean remux");
    assert_eq!(artifact.bytes, input);
}

/// RED LINE (constraint 7, both sides pinned): an H.264 input is LEGAL
/// to decode away from (thumbnail + transcode-to-VP9 accepted) and
/// REFUSED as a stream-copy remux target.
#[test]
fn h264_input_decode_away_legal_stream_copy_refused() {
    let av = lane(
        "stagegolden ffmpeg/thumbnail-png",
        "golden ffprobe/probe-h264-mp4-json",
    );
    let input = fixture("tiny-h264.mp4");
    let adapter = av.adapter();
    let census = adapter.probe_av(&input).expect("h264 census");
    assert_eq!(census.red_codec(), Some("h264"));

    // thumbnail: decode-away is LEGAL
    let thumb = adapter
        .thumbnail(&input, &census)
        .expect("h264 thumbnail decodes away")
        .expect("video input yields a frame");
    assert_eq!(&thumb.bytes[..4], b"\x89PNG", "real captured PNG golden");
    assert_eq!(thumb.producer.engine, FFMPEG_TOOL);
    assert_eq!(
        thumb.producer.tier,
        austringer_media::executor::AdapterTier::Subprocess
    );

    // transcode away: LEGAL
    let spec = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps::default(),
        trim: None,
    };
    assert!(adapter.transcode(&input, &census, &spec).is_ok());

    // stream-copy remux: REFUSED, typed, naming the RED payload
    let remux = TranscodeSpec {
        target: Target::RemuxWebm,
        caps: Caps::default(),
        trim: None,
    };
    match adapter.transcode(&input, &census, &remux) {
        Err(MediaError::UnsupportedFeature {
            format,
            feature,
            reason,
        }) => {
            assert_eq!(format, FormatId::Webm);
            assert_eq!(feature, "h264");
            assert!(
                reason.contains("RED"),
                "reason cites the licensing anchor: {reason}"
            );
        }
        other => panic!("expected the typed RED remux refusal, got {other:?}"),
    }
}

/// EXTRACT: the h264 census + thumbnail land in the MediaStore with
/// producer disclosure (the acceptance pin).
#[test]
fn extract_av_stores_thumbnail_in_media_store_with_producer() {
    let av = lane(
        "stagegolden ffmpeg/thumbnail-png",
        "golden ffprobe/probe-h264-mp4-json",
    );
    let input = fixture("tiny-h264.mp4");
    let session = tempfile::tempdir().expect("session tempdir");
    let store = MediaStore::open(session.path()).expect("media store");
    let extracted = av
        .adapter()
        .extract_av(&input, &store)
        .expect("h264 extract");
    assert_eq!(extracted.extracted.tree["streams"][0]["codec"], "h264");
    assert_eq!(extracted.extracted.thumbnails.len(), 1);
    let thumb = extracted.thumbnail.as_ref().expect("thumbnail pair");
    assert_eq!(thumb.producer.engine, FFMPEG_TOOL);
    let stored = store
        .load(&extracted.extracted.thumbnails[0].hash)
        .expect("load stored thumbnail")
        .expect("thumbnail is in the store");
    assert_eq!(&stored[..4], b"\x89PNG");
    assert_eq!(extracted.producers.len(), 2, "ffprobe + ffmpeg disclosures");
}

/// AUDIO-ONLY: thumbnail is an honest Ok(None), never a fabricated frame.
#[test]
fn audio_only_thumbnail_is_honest_none() {
    let av = lane("sleep 0", "golden ffprobe/probe-av-json");
    let census = av.adapter().probe_av(&fixture("tiny.wav")).expect("census");
    let thumb = av
        .adapter()
        .thumbnail(&fixture("tiny.wav"), &census)
        .expect("audio-only thumbnail is not an error");
    assert!(thumb.is_none());
}

/// INJECTION GUARD: specs are typed vocabularies - filter-graph strings,
/// stringified numbers, and unknown targets are typed InvalidSpec.
#[test]
fn injection_style_specs_are_rejected_typed() {
    let bad_specs = [
        serde_json::json!({"target": "webm_vp9_opus", "trim": {"start_secs": "-filter_complex evil", "end_secs": 9.0}}),
        serde_json::json!({"target": "webm_vp9_opus", "caps": {"max_fps": "60; rm -rf /"}}),
        serde_json::json!({"target": "teapot"}),
        serde_json::json!({"target": "webm_vp9_opus", "evil_extra": true}),
        serde_json::json!({"target": "webm_vp9_opus", "trim": {"start_secs": -1.0, "end_secs": 9.0}}),
    ];
    for params in bad_specs {
        assert!(
            matches!(
                TranscodeSpec::from_params(&params),
                Err(MediaError::InvalidSpec)
            ),
            "injection-style spec rejected typed: {params}"
        );
    }
}

/// RED TARGETS: H.264/H.265/AAC targets are typed refusals naming the
/// licensing law; MP4-container remux is the declared deferred lane.
#[test]
fn red_and_deferred_targets_are_typed_refusals() {
    for (name, needle) in [
        ("h264", "RED"),
        ("hevc", "RED"),
        ("aac", "RED"),
        ("libfdk_aac", "RED"),
        ("mp4", "deferred"),
    ] {
        let params = serde_json::json!({"target": name});
        match TranscodeSpec::from_params(&params) {
            Err(MediaError::UnsupportedOperation { reason, .. }) => {
                assert!(reason.contains(needle), "{name}: {reason}");
            }
            other => panic!("{name}: expected the typed refusal, got {other:?}"),
        }
    }
}

/// CLEAN TABLE: every argv word the builder can emit is a fixed literal
/// from the digest section 4 table, a validated number, or the staging
/// tokens - the fixed-template-set pin (filter-injection guard).
#[test]
fn argv_word_universe_is_the_clean_table() {
    let targets = [
        Target::RemuxWebm,
        Target::RemuxMkv,
        Target::WebmVp9Opus,
        Target::MkvSvtav1Opus,
        Target::MkvAomAv1Opus,
        Target::MkvFfv1Flac,
        Target::Opus,
        Target::Flac,
        Target::Wav,
        Target::Mp3,
        Target::OggVorbis,
        Target::WebpLossy,
    ];
    let literals = [
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-ss",
        "-t",
        "-i",
        "-c:v",
        "-c:a",
        "-c",
        "copy",
        "-vn",
        "-vf",
        "-r",
        "-b:a",
        "-frames:v",
        "-quality",
        "-map",
        "0:a",
        "-f",
        "md5",
        "-",
        "libvpx-vp9",
        "libsvtav1",
        "libaom-av1",
        "ffv1",
        "libopus",
        "flac",
        "pcm_s16le",
        "libmp3lame",
        "libvorbis",
        "libwebp",
        "80",
        "{input}",
        "{output}",
    ];
    let mut combos = Vec::new();
    for target in targets {
        for (caps, trim) in [
            (Caps::default(), None),
            (
                Caps {
                    max_width: Some(1280),
                    max_fps: Some(30.0),
                    max_audio_kbps: Some(128),
                },
                Some(Trim {
                    start_secs: 1.5,
                    end_secs: 2.5,
                }),
            ),
        ] {
            combos.push(TranscodeSpec { target, caps, trim });
        }
    }
    for spec in combos {
        let argv = ffmpeg::target_argv(&spec, true, Some(1920)).expect("argv builds");
        for word in argv {
            let ok = literals.contains(&word.as_str())
                || word.parse::<f64>().is_ok()
                || word.ends_with("k") && word.trim_end_matches('k').parse::<f64>().is_ok()
                || word.starts_with("scale=");
            assert!(
                ok,
                "word outside the clean table: {word:?} (target {:?})",
                spec.target
            );
        }
    }
    // The audio-only gate: a video-expecting target without a video
    // stream is the typed refusal, never an argv.
    let video_only = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps::default(),
        trim: None,
    };
    assert!(matches!(
        ffmpeg::target_argv(&video_only, false, None),
        Err(MediaError::UnsupportedOperation { .. })
    ));
}

/// PCM-MD5: the lossless decode-hash hook parses the real captured
/// digest out of tool stdout (leading noise tolerated).
#[test]
fn pcm_md5_parses_the_real_captured_digest() {
    let av = lane("golden ffmpeg/pcm-md5", "sleep 0");
    let digest = av
        .adapter()
        .pcm_md5(&fixture("tiny-vp9.webm"))
        .expect("pcm md5 runs");
    assert_eq!(digest, "MD5=0e0344ab90750a8dbcb7b2274096a081");
}

/// TOOL ABSENT (the default): typed ToolUnavailable with the manifest
/// install hint - nothing spawns.
#[test]
fn absent_ffprobe_is_typed_unavailable_with_hint() {
    let temp = tempfile::tempdir().expect("tempdir");
    let tools = ToolExecutor::new(ToolConfig {
        enabled: BTreeMap::new(),
        install_enabled: false,
        session_temp: temp.path().to_path_buf(),
        env: BTreeMap::new(),
    });
    let adapter = ffmpeg::Adapter::new(&tools);
    match adapter.probe_av(b"RIFF....WEBM") {
        Err(MediaError::ToolUnavailable { tool, install_hint }) => {
            assert_eq!(tool, FFPROBE_TOOL);
            assert!(install_hint.contains("opt-in"), "hint: {install_hint}");
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }
}

/// NON-ZERO EXIT: typed error, and no partial artifact or staged file
/// survives a failed run.
#[test]
fn nonzero_exit_is_typed_and_staging_is_cleaned() {
    let av = lane("exit 7 boom exploded", "golden ffprobe/probe-webm-json");
    let input = fixture("tiny-vp9.webm");
    let adapter = av.adapter();
    let census = adapter.probe_av(&input).expect("census");
    let spec = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps::default(),
        trim: None,
    };
    match adapter.transcode(&input, &census, &spec) {
        Err(MediaError::NonZeroExit {
            code, stderr_head, ..
        }) => {
            assert_eq!(code, 7);
            assert!(stderr_head.contains("boom"));
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
    let leftovers = std::fs::read_dir(&av.tools.config().session_temp)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("m17-"))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "staged files are cleaned on failure");
}

/// LOSSY DURATION TOLERANCE: max(1 frame, 1%) - inside passes, beyond
/// fails with the exact pointer (pure unit pin over synthesized
/// censuses).
#[test]
fn verify_transcode_duration_tolerance_is_max_one_frame_or_one_percent() {
    let base = r#"{"streams": [{"index": 0, "codec_type": "video", "codec_name": "vp9", "avg_frame_rate": "10/1"}], "format": {"format_name": "matroska,webm", "duration": "2.000000"}}"#;
    let after_inside = r#"{"streams": [{"index": 0, "codec_type": "video", "codec_name": "vp9", "avg_frame_rate": "10/1"}], "format": {"format_name": "matroska,webm", "duration": "2.019000"}}"#;
    let after_beyond = r#"{"streams": [{"index": 0, "codec_type": "video", "codec_name": "vp9", "avg_frame_rate": "10/1"}], "format": {"format_name": "matroska,webm", "duration": "2.110000"}}"#;
    let spec = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps::default(),
        trim: None,
    };
    let before = ffmpeg::AvProbe::parse(base).expect("before census");
    // 1 frame at 10 fps = 0.1 s; 1% of 2 s = 0.02 s -> tolerance 0.1 s
    let inside = ffmpeg::AvProbe::parse(after_inside).expect("inside census");
    assert_eq!(
        ffmpeg::verify_transcode(&before, &inside, &spec).expect("verify runs"),
        austringer_media::diff::RoundTripVerdict::Pass
    );
    let beyond = ffmpeg::AvProbe::parse(after_beyond).expect("beyond census");
    assert!(matches!(
        ffmpeg::verify_transcode(&before, &beyond, &spec).expect("verify runs"),
        austringer_media::diff::RoundTripVerdict::Fail(_)
    ));
}

/// WRONG CODEC: the after-census carrying a codec the target does not
/// declare fails the probe-diff (declared-targets law).
#[test]
fn verify_transcode_refuses_undeclared_codec() {
    let before = ffmpeg::AvProbe::parse(
        r#"{"streams": [{"index": 0, "codec_type": "video", "codec_name": "h264", "avg_frame_rate": "10/1"}], "format": {"format_name": "mov,mp4", "duration": "2.000000"}}"#,
    )
    .expect("before");
    let after = ffmpeg::AvProbe::parse(
        r#"{"streams": [{"index": 0, "codec_type": "video", "codec_name": "h264", "avg_frame_rate": "10/1"}], "format": {"format_name": "matroska,webm", "duration": "2.000000"}}"#,
    )
    .expect("after");
    let spec = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps::default(),
        trim: None,
    };
    assert!(matches!(
        ffmpeg::verify_transcode(&before, &after, &spec).expect("verify runs"),
        austringer_media::diff::RoundTripVerdict::Fail(_)
    ));
}

/// VIDEO TARGET ON AUDIO-ONLY INPUT: typed refusal - a vp9 encode of a
/// silent stream is not a lane this adapter pretends to have.
#[test]
fn video_target_on_audio_only_input_is_typed_refusal() {
    let av = lane("stagecopy", "golden ffprobe/probe-av-json");
    let input = fixture("tiny.wav");
    let adapter = av.adapter();
    let census = adapter.probe_av(&input).expect("census");
    let spec = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps::default(),
        trim: None,
    };
    match adapter.transcode(&input, &census, &spec) {
        Err(MediaError::UnsupportedOperation { reason, .. }) => {
            assert!(
                reason.contains("video"),
                "names the missing video stream: {reason}"
            );
        }
        other => panic!("expected the typed audio-only refusal, got {other:?}"),
    }
}
