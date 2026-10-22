//! The step generate lane: the typed spec becomes the in-crate builder
//! script, freecadcmd executes it into a .step (AP214-flavored export
//! with the FILE_NAME clock pinned to the determinism epoch by the
//! script itself). A non-zero exit or timeout yields NO partial
//! artifact - the output file is read only after a zero-exit run.

use super::TOOL;
use super::script::builder_script;
use super::spec::StepSpec;
use crate::error::MediaError;
use crate::executor::{OpOutput, Produced};
use crate::generate::GenerationSpec;
use crate::tool::{ToolCall, ToolExecutor};

const SCRIPT_FILE: &str = "step-build/build.py";
const OUTPUT_FILE: &str = "step-build/out.step";

/// Generate .step bytes from a validated spec through freecadcmd.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec violates the step
/// contract; the routed [`ToolError`] failures otherwise. A failing or
/// absent tool never yields bytes.
pub fn generate(tools: &ToolExecutor, spec: &GenerationSpec) -> Result<Produced, MediaError> {
    let step = StepSpec::from_generation(spec)?;
    gate(tools)?;
    let root = &tools.config().session_temp;
    let script = root.join(SCRIPT_FILE);
    let output = root.join(OUTPUT_FILE);
    let directory = script.parent().unwrap_or(root);
    std::fs::create_dir_all(directory).map_err(staging_failure)?;
    std::fs::write(&script, builder_script(&step)).map_err(staging_failure)?;
    let mut call = ToolCall::new(TOOL, "step-build");
    call.vars
        .insert("script".to_owned(), script.display().to_string());
    call.vars
        .insert("output".to_owned(), output.display().to_string());
    let run = tools.run(&call).map_err(MediaError::from)?;
    let bytes = std::fs::read(&output).map_err(staging_failure)?;
    Ok(Produced::new(OpOutput::Generate(bytes), run.producer()))
}

/// The plan's default-disabled gate: refuse typed before any staging
/// when freecadcmd is not opted in and installed (probe and the pure
/// extract lane never pass through here).
pub(super) fn gate(tools: &ToolExecutor) -> Result<(), MediaError> {
    match tools.availability(TOOL) {
        crate::tool::ToolAvailability::Available { .. } => Ok(()),
        crate::tool::ToolAvailability::Unavailable { install_hint } => {
            Err(MediaError::ToolUnavailable {
                tool: TOOL.to_owned(),
                install_hint,
            })
        }
    }
}

/// The session-staging failure mapping shared with the verify lane.
pub(super) fn staging_failure(detail: std::io::Error) -> MediaError {
    MediaError::ToolUnavailable {
        tool: TOOL.to_owned(),
        install_hint: format!("session staging failed: {detail}"),
    }
}
