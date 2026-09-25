//! The independent spec->tree projection (M5 provenance rule): the
//! dump-contract tree a spec's scene must extract to, derived WITHOUT
//! the extractor, the builder script, or the mock's blender model. The
//! primitive topology table and the TRS compose here are the adapter's
//! oracle; the python template and the mocktool stand-in carry their
//! own copies (three-way triangulation, pinned by the round-trip
//! suite).

use serde_json::{json, Value};

use super::spec::{BlendSpec, ObjectSpec, Primitive};
use super::DUMP_SCHEMA;

/// Blender's factory-default world color (dumped when the spec sets no
/// background).
const FACTORY_WORLD: [f64; 4] = [0.05, 0.05, 0.05, 1.0];

/// The canonical dump tree for `spec`.
#[must_use]
pub fn spec_tree(spec: &BlendSpec) -> Value {
    let objects: Vec<Value> = spec.objects.iter().map(object_tree).collect();
    let has_objects = !objects.is_empty();
    json!({
        "action_count": 0,
        "collections": {
            "depth": if has_objects { 1 } else { 0 },
            "names": if has_objects { vec![spec.collection.clone()] } else { Vec::<String>::new() },
        },
        "objects": objects,
        "scenes": ["Scene"],
        "schema": DUMP_SCHEMA,
        "world": {
            "background": spec.world_background.unwrap_or(FACTORY_WORLD),
            "name": "World",
        },
    })
}

fn object_tree(object: &ObjectSpec) -> Value {
    let (verts, edges, faces) = stats(object.primitive);
    let slots: Vec<Value> = object.material.iter().map(|material| {
        json!({
            "base_color": material.base_color,
            "metallic": material.metallic,
            "name": format!("{}-material", object.name),
            "roughness": material.roughness,
        })
    }).collect();
    json!({
        "material_slots": slots,
        "mesh": {"edges": edges, "faces": faces, "verts": verts},
        "modifiers": [],
        "name": object.name,
        "parent": null,
        "type": "MESH",
        "world_transform": mat4(&object.transform),
    })
}

/// Pinned per-primitive topology for the canonical from_pydata
/// constructions (verts, edges, faces).
#[must_use]
pub fn stats(primitive: Primitive) -> (u64, u64, u64) {
    match primitive {
        Primitive::Plane => (4, 4, 1),
        Primitive::Cube => (8, 12, 6),
        Primitive::Sphere => (114, 240, 128),
        Primitive::Cylinder => (32, 48, 18),
        Primitive::Cone => (17, 32, 17),
        Primitive::Icosphere => (12, 30, 20),
    }
}

/// Compose the world matrix `T * Rz * Ry * Rx * S` (column-vector
/// convention, XYZ euler order - Blender's default euler mode).
fn mat4(transform: &super::spec::Transform) -> Vec<Vec<f64>> {
    let (sx, cx) = transform.rotation_euler[0].sin_cos();
    let (sy, cy) = transform.rotation_euler[1].sin_cos();
    let (sz, cz) = transform.rotation_euler[2].sin_cos();
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
    let mut matrix = mul(&mul(&rz, &ry), &rx);
    for (row_index, row) in matrix.iter_mut().enumerate().take(3) {
        for (column, cell) in row.iter_mut().enumerate().take(3) {
            *cell *= transform.scale[column];
        }
        row[3] = transform.location[row_index];
    }
    matrix.iter().map(|row| row.to_vec()).collect()
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
