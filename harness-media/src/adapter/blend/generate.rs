//! The blend generate lane: the typed spec becomes the in-crate builder
//! script, headless Blender (`--background --factory-startup --python
//! build.py -- <out>`) executes it into a .blend. A non-zero exit or
//! timeout yields NO partial artifact - the output file is read only
//! after a zero-exit run.

use super::TOOL;
use super::script::builder_script;
use super::spec::BlendSpec;
use crate::error::MediaError;
use crate::executor::{OpOutput, Produced};
use crate::generate::GenerationSpec;
use crate::tool::{ToolCall, ToolExecutor};

const SCRIPT_FILE: &str = "blend-build/build.py";
const OUTPUT_FILE: &str = "blend-build/out.blend";

/// Generate .blend bytes from a validated spec through headless
/// Blender.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec violates the blend
/// contract; the routed [`ToolError`] failures otherwise. A failing or
/// absent tool never yields bytes.
pub fn generate(tools: &ToolExecutor, spec: &GenerationSpec) -> Result<Produced, MediaError> {
    let blend = BlendSpec::from_generation(spec)?;
    super::extract::unavailable(tools)?;
    let root = &tools.config().session_temp;
    let script = root.join(SCRIPT_FILE);
    let output = root.join(OUTPUT_FILE);
    let directory = script.parent().unwrap_or(root);
    std::fs::create_dir_all(directory).map_err(spawn_failure)?;
    std::fs::write(&script, builder_script(&blend)).map_err(spawn_failure)?;
    let mut call = ToolCall::new(TOOL, "scene-build");
    call.vars
        .insert("script".to_owned(), script.display().to_string());
    call.vars
        .insert("output".to_owned(), output.display().to_string());
    let run = tools.run(&call).map_err(MediaError::from)?;
    let bytes = std::fs::read(&output).map_err(spawn_failure)?;
    Ok(Produced::new(OpOutput::Generate(bytes), run.producer()))
}

fn spawn_failure(detail: std::io::Error) -> MediaError {
    MediaError::ToolUnavailable {
        tool: TOOL.to_owned(),
        install_hint: format!("session staging failed: {detail}"),
    }
}
