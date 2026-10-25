//! The step round-trip gate (plan M20): freecadcmd re-reads the
//! produced bytes through the topology-dump script and the census
//! diffs against the INDEPENDENT expected tree (from
//! [`StepSpec::topology_tree`](super::StepSpec)). The FILE_NAME
//! wall-clock stamp never enters the census, so OCCT's stamping cannot
//! break the diff (byte identity is explicitly NOT the gate - the plan
//! strips it by design).

use std::collections::BTreeSet;

use serde_json::Value;

use super::script::{DUMP_BEGIN, DUMP_END, dump_script};
use super::{DUMP_SCHEMA, TOOL};
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use crate::executor::{OpOutput, Produced, ProducerInfo};
use crate::tool::{ToolCall, ToolExecutor};

const INPUT_FILE: &str = "step-dump/in.step";
const SCRIPT_FILE: &str = "step-dump/dump.py";

/// The step diff profile: floats at eps 1e-4 (covers OCCT's numeric
/// volume integration and BoundBox gap at fixture scale on the real
/// lane; the hermetic lane matches exactly); solids are ORDERED-exact
/// (spec-driven writes are ordered - the M5 rule).
#[must_use]
pub fn profile() -> Tolerances {
    Tolerances {
        float_epsilon: 1e-4,
        unordered_at: BTreeSet::new(),
        ignored_at: BTreeSet::new(),
    }
}

/// Verify produced .step bytes against the independent expected
/// topology tree, through the freecadcmd-backed re-read.
///
/// # Errors
/// The routed tool/parse failures, or
/// [`MediaError::DiffTooLarge`] when the diff caps trip. A non-empty
/// diff is the [`RoundTripVerdict::Fail`] verdict, never an error.
pub fn verify(
    tools: &ToolExecutor,
    expected: &Value,
    bytes: &[u8],
) -> Result<Produced, MediaError> {
    let (census, producer) = dump_run(tools, bytes)?;
    let diff = structural_diff(expected, &census, &profile())?;
    let verdict = if diff.is_empty() {
        RoundTripVerdict::Pass
    } else {
        RoundTripVerdict::Fail(diff)
    };
    Ok(Produced::new(OpOutput::Verify(verdict), producer))
}

/// Stage `bytes`, run the dump script, and parse the census between
/// the markers (versioned schema guard - the mock-drift law pin).
///
/// # Errors
/// The routed tool failures, or [`MediaError::ParseFailure`] when the
/// stdout contract (markers / JSON / schema) is violated.
pub fn read_topology_census(tools: &ToolExecutor, bytes: &[u8]) -> Result<Value, MediaError> {
    dump_run(tools, bytes).map(|(census, _)| census)
}

fn dump_run(tools: &ToolExecutor, bytes: &[u8]) -> Result<(Value, ProducerInfo), MediaError> {
    super::generate::gate(tools)?;
    let root = &tools.config().session_temp;
    let input = root.join(INPUT_FILE);
    let script = root.join(SCRIPT_FILE);
    let directory = script.parent().unwrap_or(root);
    std::fs::create_dir_all(directory).map_err(super::generate::staging_failure)?;
    std::fs::write(&input, bytes).map_err(super::generate::staging_failure)?;
    std::fs::write(&script, dump_script()).map_err(super::generate::staging_failure)?;
    let mut call = ToolCall::new(TOOL, "topology-dump");
    call.vars
        .insert("input".to_owned(), input.display().to_string());
    call.vars
        .insert("script".to_owned(), script.display().to_string());
    let run = tools.run(&call).map_err(MediaError::from)?;
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let census = between_markers(&stdout).ok_or(MediaError::ParseFailure)?;
    let tree: Value = serde_json::from_str(census).map_err(|_| MediaError::ParseFailure)?;
    if tree.get("schema").and_then(Value::as_str) != Some(DUMP_SCHEMA) {
        return Err(MediaError::ParseFailure);
    }
    Ok((tree, run.producer()))
}

fn between_markers(stdout: &str) -> Option<&str> {
    let start = stdout.find(DUMP_BEGIN)? + DUMP_BEGIN.len();
    let end = stdout[start..].find(DUMP_END)? + start;
    stdout[start..end]
        .trim_matches(|ch: char| ch == '\n' || ch == '\r')
        .into()
}
