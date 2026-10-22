//! glTF/GLB extraction: the typed compressed-variant refusal, then the
//! canonical tree + scene-outline projection built through the
//! khronos-ecosystem `gltf` reader (validated parse; import-free).
//!
//! Canonical tree shape (serde sorted-key; floats are f32-as-f64):
//! `{accessor_sizes, animations, asset{generator,version},
//! buffer_view_sizes, container{bin_bytes?,kind}, materials[{alpha_mode,
//! base_color_factor,name}], nodes[{children,mesh,primitives[{attributes,
//! has_indices,material}],rotation,scale,translation}],
//! scene{name,roots}}`. Nodes/materials are name-keyed views (anonymous
//! entries keyed `#<index>`); TRS is null when absent; matrix transforms
//! are projected through their decomposed TRS form.

use std::time::Instant;

use gltf::Document;
use serde_json::{Map, Value};

use crate::adapter::gltf_container::{GlbChunks, glb_chunks};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;

/// Scene-hierarchy walk depth cap (also the cycle guard).
const MAX_SCENE_DEPTH: usize = 64;

/// The compressed glTF extensions the pure tier refuses whole.
const REFUSED_EXTENSIONS: [&str; 2] = ["KHR_draco_mesh_compression", "EXT_meshopt_compression"];

/// Per-format text projection bound (the M1 hard ceiling is 2000 chars;
/// the glTF outline defaults lower, per the contract).
const OUTLINE_MAX_CHARS: usize = 1200;

/// Extract: container-aware parse, compressed-variant refusal, canonical
/// tree, bounded scene outline. Budgets are honored (wall-clock deadline
/// + total-size cap; tripping either is the typed `ExtractTooLarge`).
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let deadline = Instant::now() + budgets.deadline;
    let (json, container) = if bytes.starts_with(b"glTF") {
        let GlbChunks { json, bin_len } = glb_chunks(bytes)?;
        (
            json,
            serde_json::json!({"bin_bytes": bin_len, "kind": "glb"}),
        )
    } else {
        (bytes, serde_json::json!({"kind": "gltf"}))
    };
    refuse_compressed(json)?;
    let document = gltf::Gltf::from_slice(json)
        .map_err(|_| MediaError::ParseFailure)?
        .document;
    let tree = tree(&document, container, deadline)?;
    let text_projection = outline(&tree, OUTLINE_MAX_CHARS);
    Ok(Extracted {
        tree,
        text_projection,
        thumbnails: Vec::new(),
    })
}

/// Compressed primitives are refused whole, never mangled: the pure tier
/// decodes no draco/meshopt streams, so an honest typed refusal beats a
/// fake partial extraction. (Checked before the `gltf` parse because that
/// parse rejects unknown required extensions with a far less actionable
/// error.)
fn refuse_compressed(json: &[u8]) -> Result<(), MediaError> {
    let Value::Object(root) =
        serde_json::from_slice::<Value>(json).map_err(|_| MediaError::ParseFailure)?
    else {
        return Err(MediaError::ParseFailure);
    };
    if let Some(Value::Array(required)) = root.get("extensionsRequired") {
        for extension in required {
            if let Value::String(name) = extension
                && REFUSED_EXTENSIONS.contains(&name.as_str())
            {
                return Err(MediaError::UnsupportedFeature {
                    format: FormatId::Gltf,
                    feature: name.clone(),
                    reason: "compressed primitives are not decoded in the pure tier; refusing \
                             whole rather than extracting a mangled subset"
                        .into(),
                });
            }
        }
    }
    Ok(())
}

/// The bounded scene outline (the text-projection form any model can
/// consume): asset + container line, scene roots, node census, material
/// census, structural counts.
fn outline(tree: &Value, max_chars: usize) -> String {
    let mut lines = Vec::new();
    let version = tree["asset"]["version"].as_str().unwrap_or("?");
    let kind = tree["container"]["kind"].as_str().unwrap_or("?");
    match tree["container"]["bin_bytes"].as_u64() {
        Some(bin) => lines.push(format!("gltf {version} {kind} bin={bin}B")),
        None => lines.push(format!("gltf {version} {kind}")),
    }
    let roots = tree["scene"]["roots"].as_array().map_or(0, Vec::len);
    match tree["scene"]["name"].as_str() {
        Some(name) => lines.push(format!("scene {name}: {roots} root(s)")),
        None => lines.push(format!("scene: {roots} root(s)")),
    }
    let names: Vec<&str> = tree["nodes"]
        .as_array()
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| node["name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    if !names.is_empty() {
        lines.push(format!("nodes: {}", names.join(", ")));
    }
    let materials: Vec<&str> = tree["materials"]
        .as_array()
        .map(|materials| {
            materials
                .iter()
                .filter_map(|material| material["name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    lines.push(format!(
        "materials: {} ({})",
        materials.len(),
        materials.join(", ")
    ));
    lines.push(format!(
        "animations: {}; accessors: {}; views: {}",
        tree["animations"].as_u64().unwrap_or_default(),
        tree["accessor_sizes"].as_array().map_or(0, Vec::len),
        tree["buffer_view_sizes"].as_array().map_or(0, Vec::len),
    ));
    truncate_chars(&lines.join("\n"), max_chars)
}

/// Truncate to at most `max_chars` CHARS on a char boundary.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut end = text
        .char_indices()
        .map(|(offset, _)| offset)
        .take(max_chars)
        .last()
        .unwrap_or_default();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

fn tree(document: &Document, container: Value, deadline: Instant) -> Result<Value, MediaError> {
    let root = document.as_json();
    let materials: Vec<Value> = document
        .materials()
        .enumerate()
        .map(|(index, material)| {
            serde_json::json!({
                "alpha_mode": match material.alpha_mode() {
                    gltf::material::AlphaMode::Blend => "BLEND",
                    gltf::material::AlphaMode::Mask => "MASK",
                    gltf::material::AlphaMode::Opaque => "OPAQUE",
                },
                "base_color_factor": material.pbr_metallic_roughness().base_color_factor(),
                "name": material.name().map_or_else(|| format!("#{index}"), str::to_string),
            })
        })
        .collect();
    let accessor_sizes: Vec<usize> = document
        .accessors()
        .map(|accessor| accessor.size() * accessor.count())
        .collect();
    let buffer_view_sizes: Vec<u64> = root
        .buffer_views
        .iter()
        .map(|view| view.byte_length.0)
        .collect();
    let mut nodes = Vec::new();
    let scene = match document.default_scene() {
        Some(scene) => {
            for root_node in scene.nodes() {
                node_tree(root_node, root, &mut nodes, 1, deadline)?;
            }
            serde_json::json!({
                "name": scene.name(),
                "roots": scene.nodes().map(|node| name_of(&node)).collect::<Vec<_>>(),
            })
        }
        None => Value::Null,
    };
    let mut tree = Map::new();
    tree.insert(
        "accessor_sizes".into(),
        serde_json::to_value(accessor_sizes).map_err(|_| MediaError::ParseFailure)?,
    );
    tree.insert(
        "animations".into(),
        Value::from(document.animations().count()),
    );
    tree.insert(
        "asset".into(),
        serde_json::json!({"generator": root.asset.generator, "version": root.asset.version}),
    );
    tree.insert(
        "buffer_view_sizes".into(),
        serde_json::to_value(buffer_view_sizes).map_err(|_| MediaError::ParseFailure)?,
    );
    tree.insert("container".into(), container);
    tree.insert("materials".into(), Value::Array(materials));
    tree.insert("nodes".into(), Value::Array(nodes));
    tree.insert("scene".into(), scene);
    Ok(Value::Object(tree))
}

fn name_of(node: &gltf::scene::Node<'_>) -> String {
    node.name()
        .map_or_else(|| format!("#{}", node.index()), str::to_string)
}

/// Depth-capped recursive node view; the cap doubles as the cycle guard
/// (a cyclic hierarchy trips the typed budget, never a stack overflow).
fn node_tree(
    node: gltf::scene::Node<'_>,
    root: &gltf::json::Root,
    out: &mut Vec<Value>,
    depth: usize,
    deadline: Instant,
) -> Result<(), MediaError> {
    if depth > MAX_SCENE_DEPTH {
        return Err(MediaError::ExtractTooLarge);
    }
    if Instant::now() > deadline {
        return Err(MediaError::ExtractTooLarge);
    }
    let json_node = root
        .nodes
        .get(node.index())
        .ok_or(MediaError::ParseFailure)?;
    let primitives: Vec<Value> = node.mesh().map_or_else(Vec::new, |mesh| {
        mesh.primitives()
            .map(|primitive| {
                let material = primitive.material();
                let mut attributes: Vec<String> = primitive
                    .attributes()
                    .map(|(semantic, _)| semantic_name(&semantic))
                    .collect();
                attributes.sort();
                serde_json::json!({
                    "attributes": attributes,
                    "has_indices": primitive.indices().is_some(),
                    // The reader hands out a synthetic default Material when
                    // none is attached; only real (indexed) materials enter
                    // the tree - absence stays null, never a fake "#0".
                    "material": material.name().map(str::to_string)
                        .or_else(|| material.index().map(|index| format!("#{index}"))),
                })
            })
            .collect()
    });
    let mut children = Vec::new();
    for child in node.children() {
        children.push(name_of(&child));
    }
    out.push(serde_json::json!({
        "children": children,
        "mesh": node.mesh().map(|mesh| mesh.name().map_or_else(|| format!("#{}", mesh.index()), str::to_string)),
        "name": name_of(&node),
        "primitives": primitives,
        "rotation": json_node.rotation.as_ref().map(|quaternion| quaternion.0),
        "scale": json_node.scale,
        "translation": json_node.translation,
    }));
    for child in node.children() {
        node_tree(child, root, out, depth + 1, deadline)?;
    }
    Ok(())
}

/// Canonical semantic names, set-suffixed (`POSITION`, `TEXCOORD_1`, ...).
fn semantic_name(semantic: &gltf::mesh::Semantic) -> String {
    use gltf::mesh::Semantic::{Colors, Joints, Normals, Positions, Tangents, TexCoords, Weights};
    match semantic {
        Positions => "POSITION".into(),
        Normals => "NORMAL".into(),
        Tangents => "TANGENT".into(),
        TexCoords(set) => format!("TEXCOORD_{set}"),
        Colors(set) => format!("COLOR_{set}"),
        Joints(set) => format!("JOINTS_{set}"),
        Weights(set) => format!("WEIGHTS_{set}"),
    }
}
