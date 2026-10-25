//! # Codec-clean policy (plan M17, digest sections 2+4 - standing
//! constraint 7, never softened)
//!
//! OUTPUT ONLY rule: this adapter ENCODES exclusively to GREEN targets
//! and never emits or converts into a RED codec. Decode-for-extract /
//! thumbnail / transcode-AWAY from a RED input (H.264/H.265/AAC in ->
//! clean target out) is LEGAL; stream-copy remux of RED payloads
//! (`-c copy`) is a typed REFUSAL. Both sides are test-pinned.
//!
//! The RED list (never an output; never a remux payload):
//! H.264 (libx264 - GPL + commercial pools), H.265/HEVC (libx265 -
//! pools + royalty escalation), AAC (native aac is content-encumbered;
//! libfdk_aac makes ffmpeg `--enable-nonfree` and unredistributable),
//! Theora (digest RED row). MP4-container remux of clean payloads
//! (AV1/Opus/ALAC) is a DEFERRED lane, not refused forever.
//!
//! The GREEN encoder table (digest section 4): `libvpx-vp9`,
//! `libsvtav1`, `libaom-av1`, `ffv1`, `libopus` (BSD-3), `flac`
//! (native), `libmp3lame` (patents expired 2017), `libvorbis`,
//! `pcm_s16le` (the WAV lane), `libwebp` (the M28 lossy-webp redirect;
//! still-image only in v0.2).
//!
//! Filter-injection guard: specs are a TYPED vocabulary. No
//! filter-graph string, path, or flag ever crosses from a spec into
//! argv - only enum-selected encoder names and validated numbers do
//! (test-pinned over every target x cap combination).

use serde::Deserialize;
use serde_json::Value;

use crate::error::MediaError;
use crate::format::FormatId;

/// The ffmpeg manifest tool name (the transcode/thumbnail/pcm-md5
/// lanes).
pub const FFMPEG_TOOL: &str = "ffmpeg";
/// The ffprobe manifest tool name (the census lane).
pub const FFPROBE_TOOL: &str = "ffprobe";

/// RED codec_names (ffprobe vocabulary + their encoder names).
pub const RED_CODEC_NAMES: &[&str] = &[
    "h264",
    "libx264",
    "hevc",
    "h265",
    "libx265",
    "aac",
    "libfdk_aac",
    "theora",
];

/// Spec strings that name a RED or deferred target: parsed into the
/// honest typed refusal instead of a bare serde failure.
fn refused_target_reason(name: &str) -> Option<&'static str> {
    match name {
        "h264" | "libx264" | "hevc" | "h265" | "libx265" => Some(
            "H.264/H.265 targets are RED (digest 2: GPL encoders + commercial patent pools); \
             encode to a GREEN target instead (webm/libvpx-vp9, mkv/libsvtav1 or libaom-av1, \
             mkv/ffv1)",
        ),
        "aac" | "libfdk_aac" => Some(
            "AAC generation is RED (Via Licensing pool alive; libfdk_aac makes ffmpeg \
             --enable-nonfree and unredistributable); encode to libopus or libmp3lame instead",
        ),
        "theora" => Some("Theora is a digest RED row; encode to libvpx-vp9 or libaom-av1 instead"),
        "mp4" => Some(
            "MP4-container remux is a deferred lane (digest 6): the open ISO container with \
             clean AV1/Opus/ALAC payloads only - not in v0.2; remux to webm/mkv meanwhile",
        ),
        _ => None,
    }
}

/// The typed transcode target vocabulary (GREEN only, by construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// Stream-copy remux into WebM (REFUSED on any RED input stream).
    RemuxWebm,
    /// Stream-copy remux into Matroska (REFUSED on any RED input
    /// stream).
    RemuxMkv,
    /// WebM, libvpx-vp9 + libopus.
    WebmVp9Opus,
    /// Matroska, libsvtav1 + libopus.
    MkvSvtav1Opus,
    /// Matroska, libaom-av1 + libopus.
    MkvAomAv1Opus,
    /// Matroska, ffv1 + flac (archival lossless).
    MkvFfv1Flac,
    /// Ogg Opus, libopus (the M14 encode redirect).
    Opus,
    /// FLAC (native encoder; the lossless audio lane).
    Flac,
    /// WAV, pcm_s16le (the decode lane M14 redirects opus to).
    Wav,
    /// MP3, libmp3lame (patents expired 2017).
    Mp3,
    /// Ogg Vorbis, libvorbis (the M14 encode redirect).
    OggVorbis,
    /// Still-image lossy WebP, libwebp at the fixed v0.2 quality 80
    /// (the M28 lossy-encode redirect; animated stays unsupported).
    WebpLossy,
}

impl Target {
    /// The output-container/format id for typed errors and disclosure.
    #[must_use]
    pub fn format_id(self) -> FormatId {
        match self {
            Self::RemuxWebm | Self::WebmVp9Opus | Self::WebpLossy => FormatId::Webm,
            Self::RemuxMkv | Self::MkvSvtav1Opus | Self::MkvAomAv1Opus | Self::MkvFfv1Flac => {
                FormatId::Mkv
            }
            Self::Opus => FormatId::Opus,
            Self::Flac => FormatId::Flac,
            Self::Wav => FormatId::Wav,
            Self::Mp3 => FormatId::Mp3,
            Self::OggVorbis => FormatId::Ogg,
        }
    }

    /// Staged-output extension (ffmpeg infers the muxer from it).
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::RemuxWebm | Self::WebmVp9Opus | Self::WebpLossy => "webm",
            Self::RemuxMkv | Self::MkvSvtav1Opus | Self::MkvAomAv1Opus | Self::MkvFfv1Flac => "mkv",
            Self::Opus => "opus",
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::OggVorbis => "ogg",
        }
    }

    /// The ENCODER flags (the digest 4 GREEN table selection).
    #[must_use]
    pub fn encoder_flags(self) -> (Option<&'static str>, Option<&'static str>) {
        match self {
            Self::RemuxWebm | Self::RemuxMkv => (None, None),
            Self::WebmVp9Opus => (Some("libvpx-vp9"), Some("libopus")),
            Self::MkvSvtav1Opus => (Some("libsvtav1"), Some("libopus")),
            Self::MkvAomAv1Opus => (Some("libaom-av1"), Some("libopus")),
            Self::MkvFfv1Flac => (Some("ffv1"), Some("flac")),
            Self::Opus => (None, Some("libopus")),
            Self::Flac => (None, Some("flac")),
            Self::Wav => (None, Some("pcm_s16le")),
            Self::Mp3 => (None, Some("libmp3lame")),
            Self::OggVorbis => (None, Some("libvorbis")),
            Self::WebpLossy => (Some("libwebp"), None),
        }
    }

    /// The expected ffprobe codec_names a clean run of this target
    /// yields (encoder flag != codec_name: libvpx-vp9 encodes `vp9`).
    #[must_use]
    pub fn census_codecs(self) -> (Option<&'static str>, Option<&'static str>) {
        match self {
            Self::RemuxWebm | Self::RemuxMkv => (None, None),
            Self::WebmVp9Opus => (Some("vp9"), Some("opus")),
            Self::MkvSvtav1Opus | Self::MkvAomAv1Opus => (Some("av1"), Some("opus")),
            Self::MkvFfv1Flac => (Some("ffv1"), Some("flac")),
            Self::Opus => (None, Some("opus")),
            Self::Flac => (None, Some("flac")),
            Self::Wav => (None, Some("pcm_s16le")),
            Self::Mp3 => (None, Some("mp3")),
            Self::OggVorbis => (None, Some("vorbis")),
            Self::WebpLossy => (Some("webp"), None),
        }
    }

    /// This target re-encodes (video / audio / still-image) lanes.
    #[must_use]
    pub fn is_remux(self) -> bool {
        matches!(self, Self::RemuxWebm | Self::RemuxMkv)
    }

    /// The target expects a video stream in the input.
    #[must_use]
    pub fn wants_video(self) -> bool {
        matches!(
            self,
            Self::RemuxWebm
                | Self::RemuxMkv
                | Self::WebmVp9Opus
                | Self::MkvSvtav1Opus
                | Self::MkvAomAv1Opus
                | Self::MkvFfv1Flac
                | Self::WebpLossy
        )
    }
}

/// Optional caps applied on top of the codec lane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Caps {
    /// Downscale cap (only shrinks; `-vf scale=W:-2` keeps aspect).
    pub max_width: Option<u32>,
    /// Output fps cap.
    pub max_fps: Option<f64>,
    /// Audio bitrate cap, kbit/s.
    pub max_audio_kbps: Option<u32>,
}

/// Optional trim window (input-seek semantics: `-ss`/`-to`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trim {
    /// Window start, seconds (>= 0).
    pub start_secs: f64,
    /// Window end, seconds (> start).
    pub end_secs: f64,
}

/// The typed transcode spec: target + caps + trim. TRANSCODE, not
/// generation - there is no from-nothing synthesis in this adapter.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscodeSpec {
    /// The GREEN target (RED/deferred strings refuse typed).
    pub target: Target,
    /// Optional caps.
    #[serde(default)]
    pub caps: Caps,
    /// Optional trim window.
    pub trim: Option<Trim>,
}

impl TranscodeSpec {
    /// Parse + validate from spec params. Known RED/deferred target
    /// strings surface their named refusal; anything else unparsable
    /// is the typed [`MediaError::InvalidSpec`] (injection guard).
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] for malformed/injective specs;
    /// [`MediaError::UnsupportedOperation`] for known RED/deferred
    /// targets.
    pub fn from_params(params: &Value) -> Result<Self, MediaError> {
        match serde_json::from_value::<Self>(params.clone()) {
            Ok(spec) => spec.validate(),
            Err(_) => {
                if let Some(name) = params.get("target").and_then(Value::as_str)
                    && let Some(reason) = refused_target_reason(name)
                {
                    return Err(MediaError::UnsupportedOperation {
                        format: FormatId::Webm,
                        op: crate::descriptor::Operation::Generate,
                        reason: reason.to_owned(),
                    });
                }
                Err(MediaError::InvalidSpec)
            }
        }
    }

    fn validate(self) -> Result<Self, MediaError> {
        let finite = |value: f64| value.is_finite();
        if let Some(trim) = self.trim {
            let sane = trim.start_secs.is_finite()
                && trim.end_secs.is_finite()
                && trim.start_secs >= 0.0
                && trim.end_secs > trim.start_secs;
            if !sane {
                return Err(MediaError::InvalidSpec);
            }
        }
        let caps = self.caps;
        let sane = caps.max_width.is_none_or(|w| w > 0)
            && caps.max_fps.is_none_or(|fps| fps > 0.0 && finite(fps))
            && caps.max_audio_kbps.is_none_or(|kbps| kbps > 0);
        if sane {
            Ok(self)
        } else {
            Err(MediaError::InvalidSpec)
        }
    }

    /// The RED-line gate: stream-copy remux of RED payloads is refused
    /// (decode-away remains legal for every re-encoding target).
    pub(crate) fn red_remux_refusal(&self, red: &str) -> MediaError {
        MediaError::UnsupportedFeature {
            format: self.target.format_id(),
            feature: red.to_owned(),
            reason: "stream-copy remux of RED payloads (digest 2) is refused; \
                     transcode-AWAY to a GREEN target is the legal lane"
                .into(),
        }
    }

    /// The audio-only-input gate for video-expecting targets.
    pub(crate) fn no_video_refusal(&self) -> MediaError {
        MediaError::UnsupportedOperation {
            format: self.target.format_id(),
            op: crate::descriptor::Operation::Generate,
            reason: "the target expects a video stream and the input census has none; \
                     use an audio target (opus/flac/wav/mp3/ogg_vorbis)"
                .into(),
        }
    }
}
