//! The in-crate versioned python templates (plan M20): the builder
//! script (spec embedded as a single-line JSON literal, placeholder
//! `__SPEC_JSON__`) and the topology-dump script (versioned stdout
//! census between unique markers). The scripts are committed data
//! files - `include_str!` keeps the python reviewable as python while
//! the Rust side stays a thin, deterministic renderer.

use serde_json::{json, Value};

use super::spec::StepSpec;
use super::BUILD_SCHEMA;
use crate::generate::DETERMINISM_EPOCH;

const BUILD_TEMPLATE: &str = include_str!("build_script.py");
const DUMP_TEMPLATE: &str = include_str!("dump_script.py");
const SPEC_PLACEHOLDER: &str = "__SPEC_JSON__";

/// Dump-script stdout markers (the verifier scans between them).
pub const DUMP_BEGIN: &str = "HARNESS_STEP_DUMP_BEGIN";
pub const DUMP_END: &str = "HARNESS_STEP_DUMP_END";

/// The dump script handed to freecadcmd on the verify lane.
#[must_use]
pub fn dump_script() -> &'static str {
    DUMP_TEMPLATE
}

/// The builder script for `spec`: the template with the spec embedded
/// as a compact single-line JSON literal (deterministic - same spec,
/// same script bytes; the M25 byte-identical reps rely on it).
#[must_use]
pub fn builder_script(spec: &StepSpec) -> String {
    let embedded = serde_json::to_string(&spec_json(spec)).unwrap_or_else(|_| "null".to_owned());
    BUILD_TEMPLATE.replace(SPEC_PLACEHOLDER, &embedded)
}

/// The embedded-spec JSON: the typed spec plus the schema marker and
/// the determinism epoch the post-export FILE_NAME pin uses (never a
/// bare pass-through - the contract is versioned on both sides).
#[must_use]
pub fn spec_json(spec: &StepSpec) -> Value {
    let solids: Vec<Value> = spec
        .solids
        .iter()
        .map(|solid| {
            json!({
                "name": solid.name,
                "shape": solid.shape,
                "transform": {
                    "location": solid.transform.location,
                    "rotation_xyz": solid.transform.rotation_xyz,
                },
            })
        })
        .collect();
    json!({
        "epoch": DETERMINISM_EPOCH,
        "schema": BUILD_SCHEMA,
        "solids": solids,
    })
}
