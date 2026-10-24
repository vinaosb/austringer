//! The in-crate versioned python templates (plan M16): the builder
//! script (spec embedded as a single-line JSON literal, placeholder
//! `__SPEC_JSON__`) and the dump script (versioned stdout census
//! between unique markers). The scripts are committed data files -
//! `include_str!` keeps the python reviewable as python while the Rust
//! side stays a thin, deterministic renderer.

use serde_json::{Value, json};

use super::BUILD_SCHEMA;
use super::spec::BlendSpec;

const BUILD_TEMPLATE: &str = include_str!("build_script.py");
const DUMP_TEMPLATE: &str = include_str!("dump_script.py");
const SPEC_PLACEHOLDER: &str = "__SPEC_JSON__";

/// Dump-script stdout markers (the extractor scans between them).
pub const DUMP_BEGIN: &str = "HARNESS_BLEND_DUMP_BEGIN";
pub const DUMP_END: &str = "HARNESS_BLEND_DUMP_END";

/// The dump script handed to `--python` on the extract lane.
#[must_use]
pub fn dump_script() -> &'static str {
    DUMP_TEMPLATE
}

/// The builder script for `spec`: the template with the spec embedded
/// as a compact single-line JSON literal (deterministic - same spec,
/// same script bytes; the M25 byte-identical reps rely on it).
#[must_use]
pub fn builder_script(spec: &BlendSpec) -> String {
    let embedded = serde_json::to_string(&spec_json(spec)).unwrap_or_else(|_| "null".to_owned());
    BUILD_TEMPLATE.replace(SPEC_PLACEHOLDER, &embedded)
}

/// The embedded-spec JSON: the typed spec plus the schema marker the
/// mock lane validates (never a bare pass-through - the contract is
/// versioned on both sides).
#[must_use]
pub fn spec_json(spec: &BlendSpec) -> Value {
    let objects: Vec<Value> = spec
        .objects
        .iter()
        .map(|object| {
            json!({
                "material": object.material,
                "name": object.name,
                "primitive": object.primitive,
                "transform": {
                    "location": object.transform.location,
                    "rotation_euler": object.transform.rotation_euler,
                    "scale": object.transform.scale,
                },
            })
        })
        .collect();
    json!({
        "collection": spec.collection,
        "objects": objects,
        "schema": BUILD_SCHEMA,
        "world_background": spec.world_background,
    })
}
