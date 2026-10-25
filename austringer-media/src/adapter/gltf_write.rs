//! The glTF/GLB writers: spec -> `gltf-json` `Root` -> serialized bytes
//! through BOTH container writers (.gltf with an inline base64 buffer,
//! .glb through `gltf::Glb::to_vec`).
//!
//! ## Writer shape (decision record, digest-endorsed)
//!
//! NO maintained high-level glTF exporter exists: the khronos-ecosystem
//! `gltf` crate is import-only BY DESIGN (digest-verified). The endorsed
//! shape is therefore the serde-subset writer - construct the crate's own
//! `gltf_json` Serialize types, validate them with the crate's own
//! spec-validation sweep, and wrap the GLB through the crate's own
//! container writer (`Glb::to_writer/to_vec`, binary.rs L177-231). Every
//! part of the write path except the spec interpretation is thus the
//! Khronos ecosystem's own code.
//!
//! ## Determinism
//!
//! The writer embeds no clock anywhere (glTF has no timestamp field; the
//! generator string is the fixed [`GENERATOR`]): identical specs produce
//! byte-identical files (M25's repetition evals rely on this).

use std::borrow::Cow;

use base64::Engine as _;
use gltf::binary::{Glb, Header};
use gltf::json::validation::Validate;
use gltf::json::validation::{Checked, USize64};
use gltf::json::{Asset, Buffer, Index, Root, Scene};

use crate::adapter::gltf_encode::{Layout, primitive_json};
use crate::adapter::gltf_spec::{AlphaModeSpec, Container, GltfSpec, MaterialSpec, parse_spec};
use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Fixed generator string (determinism contract: no ambient state).
pub const GENERATOR: &str = "austringer-media::gltf";

/// Generate from the subset spec into either container.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    if spec.format != FormatId::Gltf {
        return Err(MediaError::InvalidSpec);
    }
    let resolved = parse_spec(&spec.params)?;
    let (mut root, bin) = build_root(&resolved)?;
    validate_clean(&root)?;
    let json = match resolved.container {
        Container::Gltf => {
            if let Some(buffer) = root.buffers.first_mut() {
                buffer.uri = Some(format!(
                    "data:application/octet-stream;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&bin)
                ));
            }
            root.to_vec().map_err(|_| MediaError::InvalidSpec)?
        }
        Container::Glb => root.to_vec().map_err(|_| MediaError::InvalidSpec)?,
    };
    match resolved.container {
        Container::Gltf => Ok(json),
        // A mesh-less spec carries no binary payload: an empty BIN chunk
        // must be omitted entirely (Khronos: GLB_EMPTY_BIN_CHUNK,
        // buffers.byteLength minimum 1).
        Container::Glb => Glb {
            header: Header {
                magic: *b"glTF",
                version: 2,
                length: 0,
            },
            json: Cow::Owned(json),
            bin: (!bin.is_empty()).then_some(Cow::Owned(bin)),
        }
        .to_vec()
        .map_err(|_| MediaError::InvalidSpec),
    }
}

/// The full gltf-json spec-validation sweep must pass before any byte is
/// emitted: index bounds, accessor/view size consistency, min/max
/// presence on POSITION accessors, and friends.
fn validate_clean(root: &Root) -> Result<(), MediaError> {
    let mut errors = 0usize;
    root.validate(root, gltf::json::Path::new, &mut |_, _| errors += 1);
    if errors == 0 {
        Ok(())
    } else {
        Err(MediaError::InvalidSpec)
    }
}

fn build_root(spec: &GltfSpec) -> Result<(Root, Vec<u8>), MediaError> {
    let mut root = Root {
        asset: Asset {
            version: "2.0".into(),
            generator: Some(GENERATOR.into()),
            ..Asset::default()
        },
        ..Root::default()
    };
    let material_index: std::collections::BTreeMap<&str, Index<gltf::json::Material>> = spec
        .materials
        .iter()
        .enumerate()
        .map(|(index, material)| (material.name.as_str(), Index::new(index as u32)))
        .collect();
    for material in &spec.materials {
        root.materials.push(material_json(material));
    }
    let mut layout = Layout {
        bin: Vec::new(),
        views: Vec::new(),
        accessors: Vec::new(),
    };
    let mesh_index: std::collections::BTreeMap<&str, Index<gltf::json::Mesh>> = spec
        .meshes
        .iter()
        .map(|mesh| (mesh.name.as_str(), Index::new(root.meshes.len() as u32)))
        .collect();
    for mesh in &spec.meshes {
        let mut primitives = Vec::new();
        for primitive in &mesh.primitives {
            primitives.push(primitive_json(primitive, &mut layout, &material_index)?);
        }
        root.meshes.push(gltf::json::Mesh {
            name: Some(mesh.name.clone()),
            primitives,
            weights: None,
            extensions: None,
            extras: Default::default(),
        });
    }
    root.buffer_views.append(&mut layout.views);
    root.accessors.append(&mut layout.accessors);
    let bin = layout.bin;
    if !bin.is_empty() {
        root.buffers.push(Buffer {
            byte_length: USize64(bin.len() as u64),
            name: None,
            uri: None,
            extensions: None,
            extras: Default::default(),
        });
    }
    root.nodes = spec
        .nodes
        .iter()
        .map(|node| {
            let rotation = match node.rotation {
                Some(quaternion) => Some(
                    crate::adapter::gltf_spec::normalized_rotation(quaternion)
                        .ok_or(MediaError::InvalidSpec)?,
                ),
                None => None,
            };
            Ok(gltf::json::Node {
                children: (!node.children.is_empty()).then(|| {
                    node.children
                        .iter()
                        .map(|child| {
                            Index::new(
                                spec.nodes
                                    .iter()
                                    .position(|candidate| &candidate.name == child)
                                    .map_or(u32::MAX, |position| position as u32),
                            )
                        })
                        .collect()
                }),
                mesh: node
                    .mesh
                    .as_ref()
                    .and_then(|name| mesh_index.get(name.as_str()).copied()),
                name: Some(node.name.clone()),
                rotation: rotation.map(gltf::json::scene::UnitQuaternion),
                scale: node.scale,
                translation: node.translation,
                ..gltf::json::Node::default()
            })
        })
        .collect::<Result<Vec<_>, MediaError>>()?;
    let root_nodes = spec
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            !spec
                .nodes
                .iter()
                .any(|parent| parent.children.iter().any(|child| child == &node.name))
        })
        .map(|(position, _)| Index::new(position as u32))
        .collect();
    root.scenes.push(Scene {
        name: None,
        nodes: root_nodes,
        extensions: None,
        extras: Default::default(),
    });
    root.scene = Some(Index::new(0));
    Ok((root, bin))
}

fn material_json(material: &MaterialSpec) -> gltf::json::Material {
    gltf::json::Material {
        name: Some(material.name.clone()),
        alpha_mode: Checked::Valid(match material.alpha_mode {
            AlphaModeSpec::Opaque => gltf::json::material::AlphaMode::Opaque,
            AlphaModeSpec::Mask => gltf::json::material::AlphaMode::Mask,
            AlphaModeSpec::Blend => gltf::json::material::AlphaMode::Blend,
        }),
        pbr_metallic_roughness: gltf::json::material::PbrMetallicRoughness {
            base_color_factor: gltf::json::material::PbrBaseColorFactor(
                material.base_color.unwrap_or([1.0, 1.0, 1.0, 1.0]),
            ),
            ..Default::default()
        },
        ..Default::default()
    }
}
