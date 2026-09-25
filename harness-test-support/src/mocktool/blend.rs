//! The mocktool blender modes (plan M16 hermetic lane): a
//! parameterized stand-in for real headless Blender while the golden
//! capture stays pending (no Blender on the capture machine - see
//! `golden/mocktool/capture.md`). Unlike fixed canned modes, this mock
//! DERIVES its scene census from the builder script's embedded spec, so
//! the hermetic round-trip triangulates three independent derivations:
//! the python builder template, this Rust mock (standing in for
//! Blender's own mesh evaluation), and the adapter's independent
//! spec->tree projection. A real-Blender golden capture replaces the
//! derivation when the tool becomes available.
//!
//! Co-design contract with the adapter (cwd = the executor's session
//! temp): `blendbuild` reads `blend-build/build.py`, validates it as
//! python, parses its `SPEC = <json>` line, and writes the census
//! fixture `blend-build/out.blend`; `blenddump` reads
//! `blend-dump/in.blend` and prints the census between the dump
//! markers. Everything else (wrong magic, missing census, broken
//! python, unknown schema) is a non-zero exit with a stderr reason -
//! the adapter surfaces that as the typed NonZeroExit.

use serde_json::{json, Value};

use super::python::validate_python;

/// Marker the builder script contract carries (must match the adapter).
const CENSUS_MARKER: &[u8] = b"HARNESS_BLEND_MOCK_CENSUS";
/// Dump-script stdout markers (must match the adapter's extractor).
const DUMP_BEGIN: &str = "HARNESS_BLEND_DUMP_BEGIN";
const DUMP_END: &str = "HARNESS_BLEND_DUMP_END";
/// 12-byte blend header the fixture carries (64-bit, little-endian,
/// version 305 - probes Certain through the M2 sniffer).
const BLEND_HEADER: &[u8] = b"BLENDER_v305";
/// Blender's factory-default world color (the dump reports it when the
/// spec sets no background).
const DEFAULT_WORLD: [f64; 4] = [0.05, 0.05, 0.05, 1.0];

/// `blendbuild`: validate the builder script, derive the census, write
/// the magic-fixture .blend.
pub fn build_mode() -> i32 {
    let script = match std::fs::read_to_string("blend-build/build.py") {
        Ok(script) => script,
        Err(error) => {
            eprintln!("mocktool: blend build script unreadable: {error}");
            return 1;
        }
    };
    if let Err(error) = validate_python(&script) {
        eprintln!("mocktool: blend build script failed python validation: {error}");
        return 1;
    }
    match build(&script) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mocktool: blend build script rejected: {error}");
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
    if spec["schema"] != "harness-blend-build/1" {
        return Err(format!("unknown build schema {}", spec["schema"]));
    }
    let census = census(&spec);
    let mut fixture = BLEND_HEADER.to_vec();
    fixture.push(b'\n');
    fixture.extend_from_slice(CENSUS_MARKER);
    fixture.push(b'\n');
    serde_json::to_vec(&census)
        .map(|mut bytes| {
            bytes.push(b'\n');
            bytes
        })
        .map_err(|e| format!("census serialize: {e}"))
        .and_then(|bytes| {
            fixture.extend_from_slice(&bytes);
            std::fs::write("blend-build/out.blend", fixture).map_err(|e| format!("write blend: {e}"))
        })
}

/// `blenddump`: emit the census carried by the fixture between the dump
/// markers (the adapter extracts between the markers, tolerant of
/// blender's own startup chatter on real runs).
pub fn dump_mode() -> i32 {
    match dump() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mocktool: blend dump failed: {error}");
            1
        }
    }
}

fn dump() -> Result<(), String> {
    let bytes =
        std::fs::read("blend-dump/in.blend").map_err(|e| format!("read blend: {e}"))?;
    if !bytes.starts_with(BLEND_HEADER) {
        return Err("input does not carry the blend 12-byte header".into());
    }
    let start = find(&bytes, CENSUS_MARKER).ok_or("blend carries no scene census")?;
    let census_start = start + CENSUS_MARKER.len() + 1;
    let census_end = bytes[census_start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| census_start + offset)
        .ok_or("census line is unterminated")?;
    let census = &bytes[census_start..census_end];
    let parsed: Value =
        serde_json::from_slice(census).map_err(|e| format!("census is not json: {e}"))?;
    if parsed["schema"] != "harness-blend-dump/1" {
        return Err(format!("unknown dump schema {}", parsed["schema"]));
    }
    println!("{DUMP_BEGIN}");
    println!("{}", String::from_utf8_lossy(census));
    println!("{DUMP_END}");
    Ok(())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Derive the dump-contract census from a build spec: the mock's own
/// Blender model (primitive mesh stats pinned per from_pydata
/// construction, transforms composed T*Rz*Ry*Rx*S like
/// matrix_world).
fn census(spec: &Value) -> Value {
    let objects: Vec<Value> = spec["objects"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|object| {
                    let primitive = object["primitive"].as_str().unwrap_or_default();
                    let (verts, edges, faces) = stats(primitive);
                    let transform = &object["transform"];
                    let slots: Vec<Value> = (!object["material"].is_null())
                        .then(|| {
                            json!({
                                "base_color": object["material"]["base_color"],
                                "metallic": object["material"]["metallic"],
                                "name": format!("{}-material", object["name"].as_str().unwrap_or_default()),
                                "roughness": object["material"]["roughness"],
                            })
                        })
                        .into_iter()
                        .collect();
                    json!({
                        "material_slots": slots,
                        "mesh": {"edges": edges, "faces": faces, "verts": verts},
                        "modifiers": [],
                        "name": object["name"],
                        "parent": null,
                        "type": "MESH",
                        "world_transform": mat4(
                            coords(&transform["location"]),
                            coords(&transform["rotation_euler"]),
                            coords(&transform["scale"]),
                        ),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let collection = spec["collection"].as_str().unwrap_or("Collection");
    let has_objects = !objects.is_empty();
    let background = spec["world_background"].as_array().map_or(
        json!(DEFAULT_WORLD),
        |values| {
            json!(values)
        },
    );
    json!({
        "action_count": 0,
        "collections": {
            "depth": if has_objects { 1 } else { 0 },
            "names": if has_objects { json!([collection]) } else { json!([]) },
        },
        "objects": objects,
        "scenes": ["Scene"],
        "schema": "harness-blend-dump/1",
        "world": {"background": background, "name": "World"},
    })
}

/// Pinned per-primitive topology (from_pydata constructions; the python
/// template in the adapter and its independent spec->tree projection
/// carry the same table - three-way triangulation).
fn stats(primitive: &str) -> (u64, u64, u64) {
    match primitive {
        "plane" => (4, 4, 1),
        "cube" => (8, 12, 6),
        "sphere" => (114, 240, 128),
        "cylinder" => (32, 48, 18),
        "cone" => (17, 32, 17),
        "icosphere" => (12, 30, 20),
        _ => (0, 0, 0),
    }
}

fn coords(value: &Value) -> [f64; 3] {
    let at = |index: usize| value.get(index).and_then(Value::as_f64).unwrap_or(0.0);
    [at(0), at(1), at(2)]
}

/// Compose the world matrix T * Rz * Ry * Rx * S (column-vector
/// convention, XYZ euler order - Blender's default).
fn mat4(location: [f64; 3], rotation: [f64; 3], scale: [f64; 3]) -> Vec<Vec<f64>> {
    let (sx, cx) = rotation[0].sin_cos();
    let (sy, cy) = rotation[1].sin_cos();
    let (sz, cz) = rotation[2].sin_cos();
    let rx = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cx, -sx, 0.0],
        [0.0, sx, cx, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let ry = [
        [cy, 0.0, sy, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sy, 0.0, cy, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let rz = [
        [cz, -sz, 0.0, 0.0],
        [sz, cz, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut m = mul(&mul(&rz, &ry), &rx);
    for (row_index, row) in m.iter_mut().enumerate().take(3) {
        for (column, cell) in row.iter_mut().enumerate().take(3) {
            *cell *= scale[column];
        }
        row[3] = location[row_index];
    }
    m.iter().map(|row| row.to_vec()).collect()
}

fn mul(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (row, out_row) in a.iter().zip(out.iter_mut()) {
        for column in 0..4 {
            out_row[column] = (0..4).map(|k| row[k] * b[k][column]).sum();
        }
    }
    out
}
