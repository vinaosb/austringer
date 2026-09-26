//! The independent spec->tree projection (M5 provenance rule): the
//! topology-census tree a spec's document must verify to, derived
//! WITHOUT the tokenizer, the builder script, or the mock's OCCT model.
//! The pinned per-primitive topology table and the analytic
//! volume/bbox formulas here are the adapter's oracle; the python
//! template and the mocktool stand-in carry their own copies (three-way
//! triangulation, pinned by the round-trip suite).
//!
//! ## Pending-real-run pins (honesty)
//!
//! The per-primitive face/edge/vertex counts are pinned from the
//! textbook B-rep of each primitive (Euler-consistent: V-E+F=2). OCCT's
//! own counts (seam/degenerate-edge bookkeeping) are implementation
//! facts this machine cannot capture - FreeCAD is absent (see the
//! task-M20 waiver) - so the table reads `pending-first-real-run` until
//! the golden capture corrects it. Volumes and bboxes are exact
//! analytic formulas; the verify profile's tolerance covers OCCT's
//! numeric-integration and bounding-box gap on the real lane.

use serde_json::{json, Value};

use super::spec::{SolidShape, StepSpec};
use super::DUMP_SCHEMA;

/// (faces, edges, vertices) per primitive - pinned table (see module
/// docs).
#[must_use]
pub fn topology(shape: &SolidShape) -> (u64, u64, u64) {
    match shape {
        SolidShape::Box { .. } => (6, 12, 8),
        SolidShape::Cylinder { .. } => (3, 3, 2),
        SolidShape::Sphere { .. } => (1, 1, 2),
    }
}

/// The analytic volume (formula order is part of the contract: the
/// python template and the mock evaluate the same association).
#[must_use]
pub fn volume(shape: &SolidShape) -> f64 {
    match shape {
        SolidShape::Box { size } => size[0] * size[1] * size[2],
        SolidShape::Cylinder { radius, height } => {
            std::f64::consts::PI * radius * radius * height
        }
        SolidShape::Sphere { radius } => 4.0 / 3.0 * std::f64::consts::PI * radius * radius * radius,
    }
}

/// The canonical topology-census tree for `spec` (the verify lane's
/// expected tree).
#[must_use]
pub fn topology_tree(spec: &StepSpec) -> Value {
    let solids: Vec<Value> = spec
        .solids
        .iter()
        .map(|solid| {
            let (faces, edges, vertices) = topology(&solid.shape);
            let (min, max) = world_bbox(&solid.shape, &solid.transform);
            json!({
                "bbox_max": max,
                "bbox_min": min,
                "edges": edges,
                "faces": faces,
                "vertices": vertices,
                "volume": volume(&solid.shape),
            })
        })
        .collect();
    json!({
        "schema": DUMP_SCHEMA,
        "solid_count": solids.len(),
        "solids": solids,
    })
}

/// The exact analytic world bbox under the placement `T * Rz * Ry *
/// Rx`: box = rotated corners; sphere = rotation-invariant interval
/// (axis term zero); cylinder = support function along each basis
/// vector with axis `a = R * z` (radial term `r*sqrt(1-a_e^2)`, axial
/// term `max(0, h*a_e)`).
fn world_bbox(shape: &SolidShape, transform: &super::spec::Transform) -> ([f64; 3], [f64; 3]) {
    let rotation = rotation_matrix(&transform.rotation_xyz);
    match shape {
        SolidShape::Box { size } => {
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for x in [0.0, size[0]] {
                for y in [0.0, size[1]] {
                    for z in [0.0, size[2]] {
                        let world = apply(&rotation, [x, y, z], &transform.location);
                        for axis in 0..3 {
                            min[axis] = min[axis].min(world[axis]);
                            max[axis] = max[axis].max(world[axis]);
                        }
                    }
                }
            }
            (min, max)
        }
        SolidShape::Sphere { radius } => intervals(*radius, 0.0, [0.0; 3], &transform.location),
        SolidShape::Cylinder { radius, height } => intervals(
            *radius,
            *height,
            [rotation[0][2], rotation[1][2], rotation[2][2]],
            &transform.location,
        ),
    }
}

/// Per-basis support intervals: `min_e = t_e - r*sqrt(1-a_e^2) +
/// min(0, h*a_e)`, `max_e = t_e + r*sqrt(1-a_e^2) + max(0, h*a_e)` (the
/// `.max(0.0)` inside the sqrt guards fp negatives near |a_e| = 1).
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
/// placement convention shared with the builder script and the mock).
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

fn apply(
    rotation: &[[f64; 3]; 3],
    point: [f64; 3],
    location: &[f64; 3],
) -> [f64; 3] {
    [
        rotation[0][0] * point[0] + rotation[0][1] * point[1] + rotation[0][2] * point[2]
            + location[0],
        rotation[1][0] * point[0] + rotation[1][1] * point[1] + rotation[1][2] * point[2]
            + location[1],
        rotation[2][0] * point[0] + rotation[2][1] * point[1] + rotation[2][2] * point[2]
            + location[2],
    ]
}
