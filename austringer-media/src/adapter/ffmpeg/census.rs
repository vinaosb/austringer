//! The ffprobe census: machine-readable JSON (`-show_format
//! -show_streams -of json`) parsed into the typed [`AvProbe`] plus its
//! canonical tree and bounded text projection. ffprobe's JSON shape is
//! a stable public contract; this module never trusts a field beyond
//! its declared type (strings parse to numbers, unknown fields drop).

use serde_json::{Map, Value, json};

use crate::error::MediaError;
use crate::extract::TEXT_PROJECTION_MAX_CHARS;

/// Stream-count bound on the census parse (a pathological container
/// fails typed, never unbounded).
pub const MAX_CENSUS_STREAMS: usize = 64;

/// Canonical rounding for float facts (fps, durations) so trees are
/// stable across print cycles.
fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn parse_f64(value: &Value) -> Option<f64> {
    value
        .as_str()
        .and_then(|text| text.parse::<f64>().ok())
        .map(round3)
}

fn parse_ratio(value: &Value) -> Option<f64> {
    let text = value.as_str()?;
    let (num, den) = text.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        return None;
    }
    Some(round3(num / den))
}

fn parse_u64(value: &Value) -> Option<u64> {
    value.as_str().and_then(|text| text.parse().ok())
}

/// One probed stream's census row.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamCensus {
    /// Container stream index.
    pub index: u32,
    /// `audio` | `video` | other (data, subtitle, ...).
    pub codec_type: String,
    /// ffprobe codec_name (`vp9`, `opus`, `pcm_s16le`, ...).
    pub codec_name: String,
    /// Coded width, video streams only.
    pub width: Option<u32>,
    /// Coded height, video streams only.
    pub height: Option<u32>,
    /// avg_frame_rate as fps.
    pub fps: Option<f64>,
    /// Stream duration, seconds.
    pub duration_secs: Option<f64>,
    /// Audio sample rate, Hz.
    pub sample_rate: Option<u32>,
    /// Audio channel count.
    pub channels: Option<u32>,
    /// Stream bitrate, bits/second.
    pub bitrate: Option<u64>,
}

/// The typed ffprobe census over one media file.
#[derive(Debug, Clone, PartialEq)]
pub struct AvProbe {
    /// Container format_name, verbatim (`matroska,webm`, `wav`, ...).
    pub format_name: String,
    /// Container-level duration, seconds.
    pub duration_secs: Option<f64>,
    /// Container-level bitrate, bits/second.
    pub bitrate: Option<u64>,
    /// Per-stream census rows, container order.
    pub streams: Vec<StreamCensus>,
    tree: Value,
    projection: String,
}

impl AvProbe {
    /// Parse the ffprobe JSON output. Leading non-JSON noise (banner
    /// text) is tolerated by parsing from the first `{`.
    ///
    /// # Errors
    /// [`MediaError::ParseFailure`] when the payload is not ffprobe
    /// JSON; [`MediaError::ExtractTooLarge`] past
    /// [`MAX_CENSUS_STREAMS`].
    pub fn parse(payload: &str) -> Result<Self, MediaError> {
        let start = payload.find('{').ok_or(MediaError::ParseFailure)?;
        let root: Value =
            serde_json::from_str(&payload[start..]).map_err(|_| MediaError::ParseFailure)?;
        let format = &root["format"];
        let format_name = format["format_name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if format_name.is_empty() {
            return Err(MediaError::ParseFailure);
        }
        let streams_json = root["streams"].as_array().ok_or(MediaError::ParseFailure)?;
        if streams_json.len() > MAX_CENSUS_STREAMS {
            return Err(MediaError::ExtractTooLarge);
        }
        let mut streams = Vec::with_capacity(streams_json.len());
        let mut rows = Vec::with_capacity(streams_json.len());
        for stream in streams_json {
            let codec_name = stream["codec_name"].as_str().unwrap_or_default().to_owned();
            let codec_type = stream["codec_type"].as_str().unwrap_or_default().to_owned();
            let census = StreamCensus {
                index: stream["index"].as_u64().unwrap_or_default() as u32,
                codec_type: codec_type.clone(),
                codec_name: codec_name.clone(),
                width: stream["width"].as_u64().and_then(|w| u32::try_from(w).ok()),
                height: stream["height"]
                    .as_u64()
                    .and_then(|h| u32::try_from(h).ok()),
                fps: parse_ratio(&stream["avg_frame_rate"]),
                duration_secs: parse_f64(&stream["duration"]),
                sample_rate: parse_u64(&stream["sample_rate"])
                    .and_then(|rate| u32::try_from(rate).ok()),
                channels: stream["channels"]
                    .as_u64()
                    .and_then(|c| u32::try_from(c).ok()),
                bitrate: parse_u64(&stream["bit_rate"]),
            };
            if codec_name.is_empty() || codec_type.is_empty() {
                return Err(MediaError::ParseFailure);
            }
            let mut row = Map::new();
            row.insert("codec".into(), json!(codec_name));
            row.insert("codec_type".into(), json!(census.codec_type));
            if let Some(bitrate) = census.bitrate {
                row.insert("bitrate".into(), json!(bitrate));
            }
            if let Some(channels) = census.channels {
                row.insert("channels".into(), json!(channels));
            }
            if let Some(duration) = census.duration_secs {
                row.insert("duration_secs".into(), json!(duration));
            }
            if let Some(fps) = census.fps {
                row.insert("fps".into(), json!(fps));
            }
            row.insert("index".into(), json!(census.index));
            if let Some(rate) = census.sample_rate {
                row.insert("sample_rate".into(), json!(rate));
            }
            if let Some(width) = census.width {
                row.insert("width".into(), json!(width));
            }
            if let Some(height) = census.height {
                row.insert("height".into(), json!(height));
            }
            rows.push(Value::Object(row));
            streams.push(census);
        }
        let duration_secs = parse_f64(&format["duration"]);
        let bitrate = parse_u64(&format["bit_rate"]);
        let mut tree = Map::new();
        tree.insert("format".into(), json!(format_name));
        if let Some(duration) = duration_secs {
            tree.insert("duration_secs".into(), json!(duration));
        }
        if let Some(rate) = bitrate {
            tree.insert("bitrate".into(), json!(rate));
        }
        tree.insert("streams".into(), Value::Array(rows));
        let probe = Self {
            format_name,
            duration_secs,
            bitrate,
            streams,
            tree: Value::Object(tree),
            projection: String::new(),
        };
        let mut probe = probe;
        probe.projection = probe.render_projection();
        Ok(probe)
    }

    fn render_projection(&self) -> String {
        let codecs: Vec<&str> = self
            .streams
            .iter()
            .map(|row| row.codec_name.as_str())
            .collect();
        let duration = self.duration_secs.unwrap_or_default();
        let minutes = (duration / 60.0) as u64;
        let seconds = (duration - f64::from(60_u32 * minutes as u32)) as u64;
        let text = format!(
            "av: {} {} stream(s) {} {}:{seconds:02}",
            self.format_name,
            self.streams.len(),
            codecs.join("+"),
            minutes,
        );
        text.chars().take(TEXT_PROJECTION_MAX_CHARS).collect()
    }

    /// The canonical census tree (sorted keys by construction).
    #[must_use]
    pub fn tree(&self) -> &Value {
        &self.tree
    }

    /// The bounded one-line text projection.
    #[must_use]
    pub fn text_projection(&self) -> &str {
        &self.projection
    }

    /// Any video stream present.
    #[must_use]
    pub fn has_video(&self) -> bool {
        self.streams.iter().any(|row| row.codec_type == "video")
    }

    /// First video stream's coded width.
    #[must_use]
    pub fn video_width(&self) -> Option<u32> {
        self.streams
            .iter()
            .find(|row| row.codec_type == "video")
            .and_then(|row| row.width)
    }

    /// First video stream's fps.
    #[must_use]
    pub fn video_fps(&self) -> Option<f64> {
        self.streams
            .iter()
            .find(|row| row.codec_type == "video")
            .and_then(|row| row.fps)
    }

    /// The first RED-listed codec in the census (constraint 7 anchor:
    /// H.264/H.265/AAC/Theora payloads), if any.
    #[must_use]
    pub fn red_codec(&self) -> Option<&str> {
        self.streams
            .iter()
            .map(|row| row.codec_name.as_str())
            .find(|name| super::policy::RED_CODEC_NAMES.contains(name))
    }
}
