//! The mocktool freecadcmd modes (plan M20 hermetic lane): a
//! parameterized stand-in for real freecadcmd while the golden capture
//! stays pending (no FreeCAD on the capture machine - see
//! `golden/mocktool/capture.md`). Unlike fixed canned modes, this mock
//! DERIVES its topology census from the builder script's embedded
//! spec, so the hermetic round-trip triangulates three independent
//! derivations: the python builder template, this Rust mock (standing
//! in for OCCT's evaluated topology), and the adapter's independent
//! spec->tree projection. A real-freecadcmd golden capture replaces the
//! derivation when the tool becomes available.
//!
//! Co-design contract with the adapter (cwd = the executor's session
//! temp): `stepbuild` reads `step-build/build.py`, validates it as
//! python, parses its `SPEC = <json>` line, and writes the
//! AP214-flavored `step-build/out.step` (FILE_NAME clock pinned to the
//! spec epoch; a block-remark census marker; a PRODUCT chain per
//! solid); `stepdump` reads `step-dump/in.step` and prints the census
//! between the dump markers. Everything else (wrong magic, missing
//! census, broken python, unknown schema) is a non-zero exit with a
//! stderr reason - the adapter surfaces that as the typed NonZeroExit.

use serde_json::{Value, json};

use super::python::validate_python;

/// Marker carrying the census inside the mock-written STEP (a Part 21
/// block remark - skipped by every conforming reader).
const CENSUS_MARKER: &str = "AUSTRINGER_STEP_MOCK_CENSUS";
/// Dump-script stdout markers (must match the adapter's verifier).
const DUMP_BEGIN: &str = "AUSTRINGER_STEP_DUMP_BEGIN";
const DUMP_END: &str = "AUSTRINGER_STEP_DUMP_END";

/// `stepbuild`: validate the builder script, derive the census, write
/// the AP214-flavored .step.
pub fn build_mode() -> i32 {
    let script = match std::fs::read_to_string("step-build/build.py") {
        Ok(script) => script,
        Err(error) => {
            eprintln!("mocktool: step build script unreadable: {error}");
            return 1;
        }
    };
    if let Err(error) = validate_python(&script) {
        eprintln!("mocktool: step build script failed python validation: {error}");
        return 1;
    }
    match build(&script) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mocktool: step build script rejected: {error}");
            1
        }
    }
}

fn build(script: &str) -> Result<(), String> {
    let spec_line = script
        .lines()
        .find(|line| line.starts_with("SPEC = "))
        .ok_or("no `SPEC = <json>` line is embedded")?;
    let spec: Value = serde_json::from_str(spec_line.trim_start_matches("SPEC = "))
        .map_err(|e| format!("embedded SPEC is not json: {e}"))?;
    if spec["schema"] != "austringer-step-build/1" {
        return Err(format!("unknown build schema {}", spec["schema"]));
    }
    let epoch = spec["epoch"].as_str().unwrap_or("");
    if epoch.is_empty() {
        return Err("spec carries no determinism epoch".into());
    }
    let census = census(&spec);
    std::fs::write("step-build/out.step", step_document(&spec, &census, epoch))
        .map_err(|e| format!("write step: {e}"))
}

/// `stepdump`: emit the census carried by the mock-written file between
/// the dump markers (the adapter extracts between the markers, tolerant
/// of freecadcmd's own startup chatter on real runs).
pub fn dump_mode() -> i32 {
    match dump() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mocktool: step dump failed: {error}");
            1
        }
    }
}

fn dump() -> Result<(), String> {
    let bytes = std::fs::read("step-dump/in.step").map_err(|e| format!("read step: {e}"))?;
    if !bytes.starts_with(b"ISO-10303-21;") {
        return Err("input does not carry the STEP magic".into());
    }
    let text = String::from_utf8_lossy(&bytes);
    let start = text
        .find(CENSUS_MARKER)
        .ok_or("step file carries no mock topology census")?
        + CENSUS_MARKER.len();
    let end = text[start..]
        .find("*/")
        .map(|offset| start + offset)
        .ok_or("census remark is unterminated")?;
    let census = text[start..end].trim();
    let parsed: Value =
        serde_json::from_str(census).map_err(|e| format!("census is not json: {e}"))?;
    if parsed["schema"] != "austringer-step-topology/1" {
        return Err(format!("unknown dump schema {}", parsed["schema"]));
    }
    println!("{DUMP_BEGIN}");
    println!("{census}");
    println!("{DUMP_END}");
    Ok(())
}

/// The mock-written AP214-flavored document: header (FILE_NAME clock
/// pinned to the spec epoch), the census block remark, and a PRODUCT
/// chain per solid.
fn step_document(spec: &Value, census: &Value, epoch: &str) -> String {
    let mut lines: Vec<String> = vec![
        "ISO-10303-21;".into(),
        "HEADER;".into(),
        "FILE_DESCRIPTION(('austringer-media mock freecadcmd (parameterized stand-in)'),'2;1');"
            .into(),
        format!(
            "FILE_NAME('out.step','{epoch}',('austringer'),('austringer'),'austringer-media','austringer','none');"
        ),
        "FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));".into(),
        "ENDSEC;".into(),
        format!("/* {CENSUS_MARKER} {census} */"),
        "DATA;".into(),
    ];
    let solids = spec["solids"].as_array().cloned().unwrap_or_default();
    let context = solids.len() * 3 + 1;
    let application = context + 1;
    for (index, solid) in solids.iter().enumerate() {
        let base = index * 3 + 1;
        let name = solid["name"]
            .as_str()
            .unwrap_or_default()
            .replace('\'', "''");
        lines.push(format!("#{base} = PRODUCT('{name}','',(),());"));
        lines.push(format!(
            "#{} = PRODUCT_DEFINITION_FORMATION('','',#{base});",
            base + 1
        ));
        lines.push(format!(
            "#{} = PRODUCT_DEFINITION('design','',#{},#{context});",
            base + 2,
            base + 1
        ));
    }
    lines.push(format!(
        "#{context} = PRODUCT_DEFINITION_CONTEXT('part definition',#{application},'design');"
    ));
    lines.push(format!(
        "#{application} = APPLICATION_CONTEXT('core data for automotive mechanical design processes');"
    ));
    lines.push("ENDSEC;".into());
    lines.push("END-ISO-10303-21;".into());
    let mut document = lines.join("\n");
    document.push('\n');
    document
}

/// Derive the topology census from a build spec: the mock's own OCCT
/// model (pinned per-primitive topology table, analytic volume, exact
/// analytic bbox - the same formulas the adapter's independent
/// projection carries; three-way triangulation).
fn census(spec: &Value) -> Value {
    let solids: Vec<Value> = spec["solids"]
        .as_array()
        .map(|entries| entries.iter().map(solid_census).collect())
        .unwrap_or_default();
    json!({
        "schema": "austringer-step-topology/1",
        "solid_count": solids.len(),
        "solids": solids,
    })
}

fn solid_census(solid: &Value) -> Value {
    let shape = &solid["shape"];
    let location = coords(&solid["transform"]["location"]);
    let rotation = coords(&solid["transform"]["rotation_xyz"]);
    let (faces, edges, vertices, volume, (min, max)) =
        if let Some(size) = shape["box"]["size"].as_array() {
            let size = [cell(size, 0), cell(size, 1), cell(size, 2)];
            let matrix = rotation_matrix(&rotation);
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for x in [0.0, size[0]] {
                for y in [0.0, size[1]] {
                    for z in [0.0, size[2]] {
                        let world = apply(&matrix, [x, y, z], &location);
                        for axis in 0..3 {
                            min[axis] = min[axis].min(world[axis]);
                            max[axis] = max[axis].max(world[axis]);
                        }
                    }
                }
            }
            (6, 12, 8, size[0] * size[1] * size[2], (min, max))
        } else if let Some(radius) = shape["sphere"]["radius"].as_f64() {
            (
                1,
                1,
                2,
                4.0 / 3.0 * std::f64::consts::PI * radius * radius * radius,
                intervals(radius, 0.0, [0.0; 3], &location),
            )
        } else {
            let radius = shape["cylinder"]["radius"].as_f64().unwrap_or_default();
            let height = shape["cylinder"]["height"].as_f64().unwrap_or_default();
            let matrix = rotation_matrix(&rotation);
            (
                3,
                3,
                2,
                std::f64::consts::PI * radius * radius * height,
                intervals(
                    radius,
                    height,
                    [matrix[0][2], matrix[1][2], matrix[2][2]],
                    &location,
                ),
            )
        };
    json!({
        "bbox_max": max,
        "bbox_min": min,
        "edges": edges,
        "faces": faces,
        "vertices": vertices,
        "volume": volume,
    })
}

fn coords(value: &Value) -> [f64; 3] {
    value.as_array().map_or([0.0; 3], |values| {
        [cell(values, 0), cell(values, 1), cell(values, 2)]
    })
}

fn cell(values: &[Value], index: usize) -> f64 {
    values.get(index).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Per-basis support intervals (the adapter's `intervals` twin): `min_e
/// = t_e - r*sqrt(1-a_e^2) + min(0, h*a_e)`, mirrored max.
fn intervals(
    radius: f64,
    height: f64,
    axis: [f64; 3],
    location: &[f64; 3],
) -> ([f64; 3], [f64; 3]) {
    let mut min = [0.0; 3];
    let mut max = [0.0; 3];
    for basis in 0..3 {
        let along = axis[basis];
        let radial = radius * (1.0 - along * along).max(0.0).sqrt();
        let sweep = height * along;
        min[basis] = location[basis] - radial + sweep.min(0.0);
        max[basis] = location[basis] + radial + sweep.max(0.0);
    }
    (min, max)
}

/// `Rz * Ry * Rx` (column-vector convention, XYZ euler order - the
/// placement convention shared with the builder script and the
/// adapter's projection).
fn rotation_matrix(euler: &[f64; 3]) -> [[f64; 3]; 3] {
    let (sx, cx) = euler[0].sin_cos();
    let (sy, cy) = euler[1].sin_cos();
    let (sz, cz) = euler[2].sin_cos();
    [
        [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
        [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
        [-sy, cy * sx, cy * cx],
    ]
}

fn apply(rotation: &[[f64; 3]; 3], point: [f64; 3], location: &[f64; 3]) -> [f64; 3] {
    [
        rotation[0][0] * point[0]
            + rotation[0][1] * point[1]
            + rotation[0][2] * point[2]
            + location[0],
        rotation[1][0] * point[0]
            + rotation[1][1] * point[1]
            + rotation[1][2] * point[2]
            + location[1],
        rotation[2][0] * point[0]
            + rotation[2][1] * point[1]
            + rotation[2][2] * point[2]
            + location[2],
    ]
}
