//! The blend extract lane: headless Blender over the in-crate dump
//! script (plan M16). The subprocess owns wall-clock enforcement (the
//! manifest timeout + kill-tree); this side owns the input size cap,
//! the marker-bounded stdout contract, the versioned schema guard, and
//! the bounded scene-outline projection.

use std::time::Instant;

use serde_json::Value;

use super::script::{dump_script, DUMP_BEGIN, DUMP_END};
use super::{DUMP_SCHEMA, TOOL};
use crate::error::MediaError;
use crate::executor::{OpOutput, Produced};
use crate::extract::{ExtractBudgets, Extracted, TEXT_PROJECTION_MAX_CHARS};
use crate::tool::{ToolAvailability, ToolCall, ToolExecutor};

const INPUT_FILE: &str = "blend-dump/in.blend";
const SCRIPT_FILE: &str = "blend-dump/dump.py";

/// Extract the scene census from blend bytes through headless Blender.
///
/// # Errors
/// [`MediaError::ParseFailure`] when the bytes are not a blend header
/// or the dump contract (markers / schema) is violated;
/// [`MediaError::ExtractTooLarge`] when the caller budgets trip;
/// the routed [`ToolError`] failures (typed `ToolUnavailable`,
/// `NonZeroExit`, `Timeout`, ...) otherwise.
pub fn extract(
    tools: &ToolExecutor,
    bytes: &[u8],
    budgets: ExtractBudgets,
) -> Result<Produced, MediaError> {
    if bytes.len() < 12 || !bytes.starts_with(b"BLENDER") {
        return Err(MediaError::ParseFailure);
    }
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    unavailable(tools)?;
    let started = Instant::now();
    let root = &tools.config().session_temp;
    let input = root.join(INPUT_FILE);
    let script = root.join(SCRIPT_FILE);
    std::fs::create_dir_all(script.parent().unwrap_or(root)).map_err(spawn_failure)?;
    std::fs::write(&input, bytes).map_err(spawn_failure)?;
    std::fs::write(&script, dump_script()).map_err(spawn_failure)?;
    let mut call = ToolCall::new(TOOL, "scene-dump");
    call.vars.insert("input".to_owned(), input.display().to_string());
    call.vars
        .insert("script".to_owned(), script.display().to_string());
    let run = tools.run(&call).map_err(MediaError::from)?;
    if started.elapsed() > budgets.deadline {
        return Err(MediaError::ExtractTooLarge);
    }
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let census = between_markers(&stdout).ok_or(MediaError::ParseFailure)?;
    if census.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let tree: Value = serde_json::from_str(census).map_err(|_| MediaError::ParseFailure)?;
    if tree.get("schema").and_then(Value::as_str) != Some(DUMP_SCHEMA) {
        return Err(MediaError::ParseFailure);
    }
    let extracted = Extracted {
        text_projection: outline(&tree),
        thumbnails: Vec::new(),
        tree,
    };
    Ok(Produced::new(OpOutput::Extract(extracted), run.producer()))
}

/// The census JSON between the dump markers (tolerant of blender's own
/// startup chatter before and after).
fn between_markers(stdout: &str) -> Option<&str> {
    let start = stdout.find(DUMP_BEGIN)? + DUMP_BEGIN.len();
    let end = stdout[start..].find(DUMP_END)? + start;
    stdout[start..end]
        .trim_matches(|ch: char| ch == '\n' || ch == '\r')
        .into()
}

/// The bounded scene-outline projection (text-only first-class).
fn outline(tree: &Value) -> String {
    let mut lines: Vec<String> = vec![format!(
        "blend scene: {} objects, {} scene(s), world {}",
        tree["objects"].as_array().map_or(0, Vec::len),
        tree["scenes"].as_array().map_or(0, Vec::len),
        serde_json::to_string(&tree["world"]["background"]).unwrap_or_default(),
    )];
    for object in tree["objects"].as_array().unwrap_or(&Vec::new()) {
        let mesh = &object["mesh"];
        let stats = if mesh.is_null() {
            "no mesh".to_owned()
        } else {
            format!(
                "{}v/{}e/{}f",
                mesh["verts"], mesh["edges"], mesh["faces"]
            )
        };
        let materials = object["material_slots"]
            .as_array()
            .map_or(0, Vec::len);
        lines.push(format!(
            "- {} ({}): {}, {} modifier(s), {} material slot(s)",
            object["name"], object["type"], stats,
            object["modifiers"].as_array().map_or(0, Vec::len),
            materials,
        ));
    }
    let mut projection = lines.join("\n");
    projection.truncate(TEXT_PROJECTION_MAX_CHARS);
    projection
}

fn spawn_failure(detail: std::io::Error) -> MediaError {
    MediaError::ToolUnavailable {
        tool: TOOL.to_owned(),
        install_hint: format!("session staging failed: {detail}"),
    }
}

/// The plan's default-disabled gate: refuse typed before any staging
/// when blender is not opted in and installed (probe stays pure).
pub(super) fn unavailable(tools: &ToolExecutor) -> Result<(), MediaError> {
    match tools.availability(TOOL) {
        ToolAvailability::Available { .. } => Ok(()),
        ToolAvailability::Unavailable { install_hint } => {
            Err(MediaError::ToolUnavailable {
                tool: TOOL.to_owned(),
                install_hint,
            })
        }
    }
}
