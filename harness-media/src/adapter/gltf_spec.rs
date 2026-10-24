//! The glTF generation subset: typed spec, strict validation, and the
//! INDEPENDENT spec->tree projection (M5 provenance rule: expected trees
//! never come from running the extractor over the writer's own output -
//! the projection is a separate function with its own size arithmetic,
//! separately tested).
//!
//! Spec shape (serde, all fields after `nodes` optional):
//! `{container: "glb"|"gltf" (default "glb"),
//!   nodes: [{name, translation?, rotation?, scale?, children?, mesh?}],
//!   meshes: [{name, primitives: [{positions, normals?, indices?,
//!                                 material?}]}],
//!   materials: [{name, base_color?, alpha_mode?}]}`.
//! Node/mesh/material references are BY NAME (unique); the writer emits
//! one scene whose roots are the nodes no other node lists as a child.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::Value;

use crate::adapter::gltf_write::GENERATOR;
use crate::error::MediaError;

/// Which container the writer emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    /// Standard glTF JSON with an inline base64 buffer.
    Gltf,
    /// Binary glTF (JSON chunk + BIN chunk).
    Glb,
}

/// Byte width of the index accessor: u16 while every index fits, u32 once
/// any vertex index exceeds 65535 (the writer and the independent
/// projection implement this same spec rule separately, on purpose).
#[must_use]
pub fn indices_byte_width(indices: &[u32]) -> usize {
    if indices
        .iter()
        .all(|index| *index <= u64::from(u16::MAX) as u32)
    {
        2
    } else {
        4
    }
}

/// glTF requires unit rotation quaternions (the Khronos validator grades
/// violations as errors), so the writer normalizes whatever the spec
/// carries; a zero-norm quaternion is degenerate and yields `None`
/// (typed `InvalidSpec` upstream). The independent projection applies
/// the same format contract.
#[must_use]
pub fn normalized_rotation(rotation: [f32; 4]) -> Option<[f32; 4]> {
    let norm = rotation
        .iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    if norm == 0.0 {
        return None;
    }
    Some([
        rotation[0] / norm,
        rotation[1] / norm,
        rotation[2] / norm,
        rotation[3] / norm,
    ])
}

/// The typed generation subset (see the module docs for the shape).
#[derive(Debug, Clone, Deserialize)]
pub struct GltfSpec {
    /// Target container; defaults to `glb`.
    #[serde(default = "default_container")]
    pub container: Container,
    /// Scene nodes, in spec order; at least one is required.
    pub nodes: Vec<NodeSpec>,
    /// Meshes referenced by node name.
    #[serde(default)]
    pub meshes: Vec<MeshSpec>,
    /// Materials referenced by primitive name.
    #[serde(default)]
    pub materials: Vec<MaterialSpec>,
}

/// One scene node with optional TRS values.
#[derive(Debug, Clone, Deserialize)]
pub struct NodeSpec {
    /// Unique node name (the tree view is name-keyed).
    pub name: String,
    /// Translation.
    #[serde(default)]
    pub translation: Option<[f32; 3]>,
    /// Rotation quaternion (x, y, z, w).
    #[serde(default)]
    pub rotation: Option<[f32; 4]>,
    /// Scale.
    #[serde(default)]
    pub scale: Option<[f32; 3]>,
    /// Child node names.
    #[serde(default)]
    pub children: Vec<String>,
    /// Mesh name this node instantiates.
    #[serde(default)]
    pub mesh: Option<String>,
}

/// A named mesh carrying primitives.
#[derive(Debug, Clone, Deserialize)]
pub struct MeshSpec {
    /// Unique mesh name.
    pub name: String,
    /// Geometry primitives, in order.
    pub primitives: Vec<PrimitiveSpec>,
}

/// One primitive: vertex data plus optional indices and material.
#[derive(Debug, Clone, Deserialize)]
pub struct PrimitiveSpec {
    /// Vertex positions (at least one vertex required).
    pub positions: Vec<[f32; 3]>,
    /// Per-vertex normals; must match `positions` in length when present.
    #[serde(default)]
    pub normals: Option<Vec<[f32; 3]>>,
    /// Triangle indices; each must reference an existing vertex.
    #[serde(default)]
    pub indices: Option<Vec<u32>>,
    /// Material name.
    #[serde(default)]
    pub material: Option<String>,
}

/// A named material with a PBR base color.
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialSpec {
    /// Unique material name.
    pub name: String,
    /// `baseColorFactor` (RGBA), defaults to opaque white.
    #[serde(default)]
    pub base_color: Option<[f32; 4]>,
    /// Alpha mode, defaults to `OPAQUE`.
    #[serde(default)]
    pub alpha_mode: AlphaModeSpec,
}

/// The three glTF alpha modes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AlphaModeSpec {
    /// `OPAQUE`.
    #[default]
    Opaque,
    /// `MASK`.
    Mask,
    /// `BLEND`.
    Blend,
}

impl AlphaModeSpec {
    /// The wire string glTF uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "OPAQUE",
            Self::Mask => "MASK",
            Self::Blend => "BLEND",
        }
    }
}

fn default_container() -> Container {
    Container::Glb
}

/// Parse and validate `params` against the subset contract; every
/// violation is the typed [`MediaError::InvalidSpec`].
pub fn parse_spec(params: &Value) -> Result<GltfSpec, MediaError> {
    let spec: GltfSpec =
        serde_json::from_value(params.clone()).map_err(|_| MediaError::InvalidSpec)?;
    if spec.nodes.is_empty() {
        return Err(MediaError::InvalidSpec);
    }
    unique_names(spec.nodes.iter().map(|node| node.name.as_str()))?;
    unique_names(spec.meshes.iter().map(|mesh| mesh.name.as_str()))?;
    unique_names(spec.materials.iter().map(|material| material.name.as_str()))?;
    let node_names: BTreeSet<&str> = spec.nodes.iter().map(|node| node.name.as_str()).collect();
    let mesh_names: BTreeSet<&str> = spec.meshes.iter().map(|mesh| mesh.name.as_str()).collect();
    let material_names: BTreeSet<&str> = spec
        .materials
        .iter()
        .map(|material| material.name.as_str())
        .collect();
    for node in &spec.nodes {
        if node
            .children
            .iter()
            .any(|child| !node_names.contains(child.as_str()))
            || node
                .mesh
                .as_ref()
                .is_some_and(|mesh| !mesh_names.contains(mesh.as_str()))
        {
            return Err(MediaError::InvalidSpec);
        }
    }
    // A glTF hierarchy is a forest: no node may be somebody's child twice.
    let mut child_refs = std::collections::BTreeSet::new();
    if !spec.nodes.iter().all(|node| {
        node.children
            .iter()
            .all(|child| child_refs.insert(child.as_str()))
    }) {
        return Err(MediaError::InvalidSpec);
    }
    for mesh in &spec.meshes {
        for primitive in &mesh.primitives {
            let vertex_count = primitive.positions.len();
            if vertex_count == 0
                || primitive
                    .normals
                    .as_ref()
                    .is_some_and(|normals| normals.len() != vertex_count)
                || primitive.indices.as_ref().is_some_and(|indices| {
                    indices.is_empty()
                        || indices.iter().any(|index| *index as usize >= vertex_count)
                })
            {
                return Err(MediaError::InvalidSpec);
            }
            if primitive
                .material
                .as_ref()
                .is_some_and(|material| !material_names.contains(material.as_str()))
            {
                return Err(MediaError::InvalidSpec);
            }
        }
    }
    Ok(spec)
}

fn unique_names<'a>(mut names: impl Iterator<Item = &'a str>) -> Result<(), MediaError> {
    let mut seen = BTreeMap::new();
    if !names.all(|name| seen.insert(name, ()).is_none()) {
        return Err(MediaError::InvalidSpec);
    }
    Ok(())
}

/// The INDEPENDENT spec->tree projection: the expected tree a generated
/// artifact must extract to, computed from the spec alone (never from the
/// writer's bytes). Mirrors the extractor's canonical form: name-keyed
/// nodes, unordered materials, accessor/view byte sizes from its own
/// count arithmetic, GLB bin size 4-byte aligned.
pub fn spec_tree(params: &Value) -> Result<Value, MediaError> {
    let spec = parse_spec(params)?;
    let mut accessor_sizes = Vec::new();
    let mut view_sizes = Vec::new();
    let mut buffer_bytes: u64 = 0;
    for mesh in &spec.meshes {
        for primitive in &mesh.primitives {
            let widths = [
                Some(12 * primitive.positions.len() as u64),
                primitive
                    .normals
                    .as_ref()
                    .map(|normals| 12 * normals.len() as u64),
                primitive
                    .indices
                    .as_ref()
                    .map(|indices| indices_byte_width(indices) as u64 * indices.len() as u64),
            ];
            for width in widths.into_iter().flatten() {
                accessor_sizes.push(width);
                view_sizes.push(width);
                buffer_bytes = buffer_bytes.div_ceil(4) * 4 + width;
            }
        }
    }
    let parented: BTreeMap<&str, ()> = spec
        .nodes
        .iter()
        .flat_map(|node| node.children.iter().map(|child| (child.as_str(), ())))
        .collect();
    let nodes: Vec<Value> = spec
        .nodes
        .iter()
        .map(|node| {
            let rotation = match node.rotation {
                Some(quaternion) => {
                    Some(normalized_rotation(quaternion).ok_or(MediaError::InvalidSpec)?)
                }
                None => None,
            };
            Ok(serde_json::json!({
                "children": node.children,
                "mesh": node.mesh,
                "name": node.name,
                "primitives": primitive_summaries(node, &spec),
                "rotation": rotation,
                "scale": node.scale,
                "translation": node.translation,
            }))
        })
        .collect::<Result<Vec<_>, MediaError>>()?;
    Ok(serde_json::json!({
        "accessor_sizes": accessor_sizes,
        "animations": 0,
        "asset": {"generator": GENERATOR, "version": "2.0"},
        "buffer_view_sizes": view_sizes,
        "container": match spec.container {
            Container::Gltf => serde_json::json!({"kind": "gltf"}),
            Container::Glb => serde_json::json!({"bin_bytes": buffer_bytes.div_ceil(4) * 4, "kind": "glb"}),
        },
        "materials": material_summaries(&spec),
        "nodes": nodes,
        "scene": {
            "name": null,
            "roots": spec.nodes.iter().map(|node| node.name.clone())
                .filter(|name| !parented.contains_key(name.as_str())).collect::<Vec<_>>()
        }
    }))
}

fn material_summaries(spec: &GltfSpec) -> Value {
    spec.materials
        .iter()
        .map(|material| {
            serde_json::json!({
                "alpha_mode": material.alpha_mode.as_str(),
                "base_color_factor": material.base_color.unwrap_or([1.0, 1.0, 1.0, 1.0]),
                "name": material.name,
            })
        })
        .collect()
}

fn primitive_summaries(node: &NodeSpec, spec: &GltfSpec) -> Value {
    let Some(mesh_name) = node.mesh.as_ref() else {
        return Value::Array(Vec::new());
    };
    let Some(mesh) = spec.meshes.iter().find(|mesh| &mesh.name == mesh_name) else {
        return Value::Array(Vec::new());
    };
    mesh.primitives
        .iter()
        .map(|primitive| {
            let mut attributes = vec!["POSITION"];
            if primitive.normals.is_some() {
                attributes.push("NORMAL");
            }
            attributes.sort_unstable();
            serde_json::json!({
                "attributes": attributes,
                "has_indices": primitive.indices.is_some(),
                "material": primitive.material,
            })
        })
        .collect()
}
