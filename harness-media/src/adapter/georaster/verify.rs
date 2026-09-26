//! The georaster round-trip gate (plan M21): run `gdalinfo -json` over
//! the produced bytes and assert the gdal-eye view (size, band type,
//! geoTransform, LAYOUT=COG when the expected tree says tiled) on top
//! of the M5 structural diff between the pure re-extract and the
//! INDEPENDENT expected tree. A failed assertion or non-empty diff is
//! the [`RoundTripVerdict::Fail`] verdict carrying the typed
//! [`StructuralDiff`]; a malformed gdalinfo census is the typed
//! `ParseFailure`, never a silent pass.

use std::collections::BTreeSet;

use serde_json::Value;

use super::extract::extract;
use super::TOOL;
use crate::diff::{structural_diff, Change, RoundTripVerdict, StructuralDiff, Tolerances};
use crate::error::MediaError;
use crate::executor::{OpOutput, Produced};
use crate::extract::ExtractBudgets;
use crate::tool::{ToolCall, ToolExecutor};

const INFO_INPUT: &str = "gdal-info/in.tif";
/// GeoTransform tolerance: f64 storage round-trips exactly; the slack
/// absorbs real-GDAL double formatting.
const GEO_EPSILON: f64 = 1e-9;

/// The georaster diff profile: floats at eps 1e-9, no unordered paths
/// (raster structure is order-exact).
#[must_use]
pub fn profile() -> Tolerances {
    Tolerances {
        float_epsilon: 1e-9,
        unordered_at: BTreeSet::new(),
        ignored_at: BTreeSet::new(),
    }
}

/// Verify produced GeoTIFF/COG bytes against the independent expected
/// tree (from [`GeoRasterSpec::canonical_tree`](super::GeoRasterSpec))
/// through gdalinfo + the pure re-extract.
///
/// # Errors
/// The routed tool failures, [`MediaError::ParseFailure`] when the
/// gdalinfo stdout is not its JSON census, or
/// [`MediaError::DiffTooLarge`] when the diff caps trip. A failed
/// assertion or non-empty diff is the [`RoundTripVerdict::Fail`]
/// verdict, never an error.
pub fn verify(
    tools: &ToolExecutor,
    expected: &Value,
    bytes: &[u8],
) -> Result<Produced, MediaError> {
    super::unavailable(tools)?;
    let input = tools.config().session_temp.join(INFO_INPUT);
    let directory = input.parent().unwrap_or(&tools.config().session_temp);
    std::fs::create_dir_all(directory).map_err(spawn_failure)?;
    std::fs::write(&input, bytes).map_err(spawn_failure)?;
    let mut call = ToolCall::new(TOOL, "info");
    call.vars
        .insert("input".to_owned(), input.display().to_string());
    let run = tools.run(&call).map_err(MediaError::from)?;
    let text = String::from_utf8_lossy(&run.stdout);
    // Tolerant first-JSON-value parse: the hermetic re-exec child is a
    // libtest binary whose harness chatter surrounds the mock's census
    // (the blend lane's marker contract, applied to a whole-stdout
    // JSON op).
    let start = text.find('{').ok_or(MediaError::ParseFailure)?;
    let census: Value = serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()
        .transpose()
        .map_err(|_| MediaError::ParseFailure)?
        .ok_or(MediaError::ParseFailure)?;
    let mut diff = gdalinfo_diff(expected, &census);
    let re_extracted = extract(bytes, ExtractBudgets::default())?;
    let structural = structural_diff(expected, &re_extracted.tree, &profile())?;
    diff.added.extend(structural.added);
    diff.removed.extend(structural.removed);
    diff.changed.extend(structural.changed);
    let verdict = if diff.is_empty() {
        RoundTripVerdict::Pass
    } else {
        RoundTripVerdict::Fail(diff)
    };
    Ok(Produced::new(OpOutput::Verify(verdict), run.producer()))
}

/// The gdalinfo assertions, as typed diff entries under a `/gdalinfo`
/// pointer namespace (size, band type, geoTransform, COG layout).
fn gdalinfo_diff(expected: &Value, census: &Value) -> StructuralDiff {
    let mut diff = StructuralDiff::default();
    let want_size = Value::Array(vec![expected["width"].clone(), expected["height"].clone()]);
    let size_ok = census["size"].as_array().is_some_and(|values| {
        values.first().and_then(Value::as_u64) == expected["width"].as_u64()
            && values.get(1).and_then(Value::as_u64) == expected["height"].as_u64()
    });
    if !size_ok {
        diff.changed
            .push(change("/gdalinfo/size", want_size, census["size"].clone()));
    }
    if census["bands"][0]["type"].as_str() != Some("Byte") {
        diff.changed.push(change(
            "/gdalinfo/band-type",
            Value::from("Byte"),
            census["bands"][0]["type"].clone(),
        ));
    }
    let want_geo = expected_geo_transform(expected);
    let actual_gt = census["geoTransform"].clone();
    let gt_ok = want_geo.as_array().is_some_and(|want| {
        actual_gt.as_array().is_some_and(|actual| {
            actual.len() == want.len()
                && actual.iter().zip(want.iter()).all(|(a, w)| match (a.as_f64(), w.as_f64()) {
                    (Some(a), Some(w)) => near(a, w),
                    _ => false,
                })
        })
    });
    if !gt_ok {
        diff.changed.push(change(
            "/gdalinfo/geoTransform",
            want_geo,
            actual_gt,
        ));
    }
    let layout = census["metadata"]["IMAGE_STRUCTURE"]["LAYOUT"].clone();
    let want_cog = expected["layout"]["tiled"].as_bool().unwrap_or(false);
    let layout_ok = if want_cog {
        layout.as_str() == Some("COG")
    } else {
        layout.as_str() != Some("COG")
    };
    if !layout_ok {
        diff.changed.push(change(
            "/gdalinfo/layout",
            Value::from(if want_cog { "COG" } else { "not COG" }),
            layout,
        ));
    }
    diff
}

fn expected_geo_transform(expected: &Value) -> Value {
    let pixel_y = expected["geo"]["pixel_size"][1].as_f64().unwrap_or(0.0);
    Value::Array(vec![
        expected["geo"]["bounds"]["minx"].clone(),
        expected["geo"]["pixel_size"][0].clone(),
        Value::from(0.0),
        expected["geo"]["bounds"]["maxy"].clone(),
        Value::from(0.0),
        Value::from(-pixel_y),
    ])
}

fn change(path: &str, expected: Value, actual: Value) -> Change {
    Change {
        path: path.to_owned(),
        expected,
        actual,
    }
}

fn near(actual: f64, want: f64) -> bool {
    (actual - want).abs() <= GEO_EPSILON * actual.abs().max(want.abs()).max(1.0)
}

fn spawn_failure(detail: std::io::Error) -> MediaError {
    MediaError::ToolUnavailable {
        tool: TOOL.to_owned(),
        install_hint: format!("session staging failed: {detail}"),
    }
}
