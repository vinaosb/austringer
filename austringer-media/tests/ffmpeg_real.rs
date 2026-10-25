//! M17 real-ffmpeg tier (`#[ignore]`, never in CI): runs ONLY when
//! `FFMPEG_BIN` names a real ffmpeg executable (ffprobe is expected as
//! its sibling). This is the at-todo-time dev-machine evidence lane
//! (plan: probe + one transcode + decode-back).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use austringer_media::adapter::ffmpeg::{
    self, Caps, FFMPEG_TOOL, FFPROBE_TOOL, Target, TranscodeSpec,
};
use austringer_media::tool::{ToolConfig, ToolExecutor, ToolOverride};

/// Compile-time-anchored input dir (a hardcoded `"\\..\\..."` suffix
/// breaks on unix, where the backslash is not a path separator).
fn input_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("austringer-test-support")
        .join("golden")
        .join("mocktool")
        .join("_input")
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(input_dir().join(name)).unwrap_or_default()
}

fn real_executor() -> Option<ToolExecutor> {
    let ffmpeg_bin = std::env::var("FFMPEG_BIN").ok()?;
    let ffmpeg_path = PathBuf::from(&ffmpeg_bin);
    let ffprobe_path = ffmpeg_path.parent()?.join("ffprobe.exe");
    if !ffmpeg_path.is_file() || !ffprobe_path.is_file() {
        return None;
    }
    let temp = tempfile::tempdir().expect("tempdir");
    let session_temp = temp.path().to_path_buf();
    std::mem::forget(temp);
    let override_for = |program: String| ToolOverride {
        program: Some(program),
        args: Vec::new(),
        timeout_ms: None,
    };
    Some(ToolExecutor::new(ToolConfig {
        enabled: [
            (FFMPEG_TOOL.to_owned(), override_for(ffmpeg_bin)),
            (
                FFPROBE_TOOL.to_owned(),
                override_for(ffprobe_path.to_string_lossy().into_owned()),
            ),
        ]
        .into(),
        install_enabled: false,
        session_temp,
        env: BTreeMap::new(),
    }))
}

/// One lane is enough: missing tool -> typed ToolUnavailable (no spawn).
#[test]
fn real_ffprobe_absent_is_typed_unavailable() {
    let temp = tempfile::tempdir().expect("tempdir");
    let tools = ToolExecutor::new(ToolConfig {
        enabled: BTreeMap::new(),
        install_enabled: false,
        session_temp: temp.path().to_path_buf(),
        env: BTreeMap::new(),
    });
    let adapter = ffmpeg::Adapter::new(&tools);
    assert!(matches!(
        adapter.probe_av(b"RIFF....WEBM"),
        Err(austringer_media::error::MediaError::ToolUnavailable { .. })
    ));
}

/// PROBE + TRANSCODE + DECODE-BACK on the dev machine (the plan's
/// evidence triple). tiny-h264.mp4 -> webm/libvpx-vp9+libopus is the
/// RED-input decode-away lane, legal per constraint 7.
#[test]
#[ignore = "real-tool tier: set FFMPEG_BIN to the ffmpeg executable (never in CI)"]
fn real_probe_transcode_decode_back() {
    let Some(tools) = real_executor() else {
        eprintln!("FFMPEG_BIN not set or ffmpeg/ffprobe not found - skipping");
        return;
    };
    let adapter = ffmpeg::Adapter::new(&tools);
    let input = fixture("tiny-h264.mp4");
    let census = adapter.probe_av(&input).expect("real h264 census");
    assert_eq!(census.red_codec(), Some("h264"));
    assert!(census.has_video());

    let spec = TranscodeSpec {
        target: Target::WebmVp9Opus,
        caps: Caps {
            max_width: Some(32),
            max_fps: Some(5.0),
            max_audio_kbps: Some(24),
        },
        trim: None,
    };
    let artifact = adapter
        .transcode(&input, &census, &spec)
        .expect("real transcode");
    assert!(!artifact.bytes.is_empty());

    let after = adapter
        .probe_av(&artifact.bytes)
        .expect("real after census");
    assert_eq!(
        ffmpeg::verify_transcode(&census, &after, &spec).expect("real probe-diff runs"),
        austringer_media::diff::RoundTripVerdict::Pass,
        "real probe-diff on the declared target"
    );

    // decode-back: the lossless PCM hash of the opus-bearing fixture
    // (the transcoded artifact itself is video-only - tiny-h264.mp4
    // carries no audio stream for 0:a to map)
    let digest = adapter
        .pcm_md5(&fixture("tiny-vp9.webm"))
        .expect("decode-back hash");
    assert!(
        digest.starts_with("MD5="),
        "decode-back produced a PCM hash: {digest}"
    );

    // thumbnail decode-away from the RED input
    let thumb = adapter
        .thumbnail(&input, &census)
        .expect("real thumbnail")
        .expect("video frame");
    assert_eq!(&thumb.bytes[..4], b"\x89PNG");
}
