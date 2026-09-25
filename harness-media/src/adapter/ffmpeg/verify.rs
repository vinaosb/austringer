//! The probe-diff verify: probe(before/after) structural diff on the
//! DECLARED targets. Lossy codecs are never byte-compared - the gate
//! is duration + stream census + codec identity, with the lossy
//! duration tolerance max(1 frame, 1%).

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use crate::diff::{structural_diff, RoundTripVerdict, Tolerances};
use crate::error::MediaError;

use super::census::AvProbe;
use super::policy::TranscodeSpec;

/// The M5 unordered-escape pointer for the stream census (container
/// stream order is not a comparable fact).
const STREAMS_POINTER: &str = "/streams";

fn expected_tree(before: &AvProbe, spec: &TranscodeSpec) -> Value {
    let target = spec.target;
    let mut streams = Vec::new();
    if target.is_remux() {
        for row in &before.streams {
            streams.push(row_shape(&row.codec_type, &row.codec_name));
        }
    } else {
        let (video, audio) = target.census_codecs();
        if let Some(codec) = video {
            streams.push(row_shape("video", codec));
        }
        if let Some(codec) = audio
            && before.streams.iter().any(|row| row.codec_type == "audio")
        {
            streams.push(row_shape("audio", codec));
        }
    }
    let mut tree = Map::new();
    if let Some(duration) = spec
        .trim
        .map(|trim| trim.end_secs - trim.start_secs)
        .or(before.duration_secs)
    {
        tree.insert("duration_secs".into(), json!(duration));
    }
    tree.insert("streams".into(), Value::Array(streams));
    Value::Object(tree)
}

fn row_shape(codec_type: &str, codec: &str) -> Value {
    json!({"codec": codec, "codec_type": codec_type})
}

fn actual_tree(after: &AvProbe) -> Value {
    let mut tree = Map::new();
    if let Some(duration) = after.duration_secs {
        tree.insert("duration_secs".into(), json!(duration));
    }
    let streams: Vec<Value> = after
        .streams
        .iter()
        .map(|row| row_shape(&row.codec_type, &row.codec_name))
        .collect();
    tree.insert("streams".into(), Value::Array(streams));
    Value::Object(tree)
}

/// The lossy duration tolerance: max(1 frame at the EFFECTIVE output
/// fps (the input fps capped by the spec's fps cap - a cap rescales
/// frame timing), 1% of the expected duration).
fn duration_epsilon(before: &AvProbe, spec: &TranscodeSpec, expected_secs: f64) -> f64 {
    let effective_fps = match (before.video_fps(), spec.caps.max_fps) {
        (Some(fps), Some(cap)) => Some(fps.min(cap)),
        (None, Some(cap)) => Some(cap),
        (fps, None) => fps,
    };
    let one_frame = effective_fps
        .filter(|fps| *fps > 0.0)
        .map_or(0.0, |fps| 1.0 / fps);
    (expected_secs / 100.0).max(one_frame)
}

/// Verify one completed transcode: the after-census must match the
/// declared target (codec identity, stream census, duration within
/// the lossy tolerance). An empty diff is the PASS verdict.
///
/// # Errors
/// [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify_transcode(
    before: &AvProbe,
    after: &AvProbe,
    spec: &TranscodeSpec,
) -> Result<RoundTripVerdict, MediaError> {
    let expected = expected_tree(before, spec);
    let epsilon = expected
        .get("duration_secs")
        .and_then(Value::as_f64)
        .map(|duration| duration_epsilon(before, spec, duration))
        .unwrap_or(1e-9);
    let mut unordered = BTreeSet::new();
    unordered.insert(STREAMS_POINTER.to_owned());
    let tolerances = Tolerances {
        float_epsilon: epsilon,
        unordered_at: unordered,
        ignored_at: BTreeSet::new(),
    };
    let diff = structural_diff(&expected, &actual_tree(after), &tolerances)?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}
