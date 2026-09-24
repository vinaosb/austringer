//! Primitive -> buffer encoding: the byte layout (one tightly packed,
//! 4-byte-aligned bufferView per accessor) and the accessor construction
//! (positions with computed min/max, normals, u16/u32 indices) feeding
//! [`gltf_write`]'s Root assembly. Emission order is canonical:
//! positions, normals, indices per primitive, meshes in spec order.

use gltf::json::{
    accessor::{GenericComponentType, Type as AccessorType},
    validation::{Checked, USize64},
    Index,
};

use crate::adapter::gltf_spec::{indices_byte_width, PrimitiveSpec};
use crate::error::MediaError;

/// Buffer layout state consumed by [`gltf_write`]'s Root assembly.
pub(super) struct Layout {
    /// The BIN-chunk bytes (or the base64 payload for .gltf).
    pub(super) bin: Vec<u8>,
    /// BufferViews in emission order.
    pub(super) views: Vec<gltf::json::buffer::View>,
    /// Accessors in emission order.
    pub(super) accessors: Vec<gltf::json::Accessor>,
}

impl Layout {
    fn push(&mut self, bytes: &[u8], target: Option<Checked<gltf::json::buffer::Target>>) -> Index<gltf::json::buffer::View> {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let offset = self.bin.len() as u64;
        self.bin.extend_from_slice(bytes);
        let index = Index::new(self.views.len() as u32);
        self.views.push(gltf::json::buffer::View {
            buffer: Index::new(0),
            byte_length: USize64(bytes.len() as u64),
            byte_offset: (offset > 0).then_some(USize64(offset)),
            byte_stride: None,
            target,
            name: None,
            extensions: None,
            extras: Default::default(),
        });
        index
    }

    fn accessor(
        &mut self,
        count: usize,
        component: Checked<GenericComponentType>,
        type_: Checked<AccessorType>,
        bounds: Option<(serde_json::Value, serde_json::Value)>,
        target: Option<Checked<gltf::json::buffer::Target>>,
        bytes: &[u8],
    ) -> Index<gltf::json::Accessor> {
        let view = self.push(bytes, target);
        let index = Index::new(self.accessors.len() as u32);
        let (min, max) = bounds.unwrap_or((serde_json::Value::Null, serde_json::Value::Null));
        self.accessors.push(gltf::json::Accessor {
            buffer_view: Some(view),
            byte_offset: None,
            count: USize64(count as u64),
            component_type: component,
            type_,
            min: if min.is_null() { None } else { Some(min) },
            max: if max.is_null() { None } else { Some(max) },
            name: None,
            normalized: false,
            sparse: None,
            extensions: None,
            extras: Default::default(),
        });
        index
    }
}

/// Encode one primitive: POSITION (min/max computed), optional NORMAL,
/// optional indices (u16 while every index fits, else u32).
pub(super) fn primitive_json(
    primitive: &PrimitiveSpec,
    layout: &mut Layout,
    materials: &std::collections::BTreeMap<&str, Index<gltf::json::Material>>,
) -> Result<gltf::json::mesh::Primitive, MediaError> {
    let float = || Checked::Valid(GenericComponentType(gltf::json::accessor::ComponentType::F32));
    let mut bytes = Vec::with_capacity(primitive.positions.len() * 12);
    for position in &primitive.positions {
        for axis in position {
            bytes.extend_from_slice(&axis.to_le_bytes());
        }
    }
    let (min, max) = bounds(primitive.positions.iter().copied().flatten());
    let positions = layout.accessor(
        primitive.positions.len(),
        float(),
        Checked::Valid(AccessorType::Vec3),
        Some((min, max)),
        Some(Checked::Valid(gltf::json::buffer::Target::ArrayBuffer)),
        &bytes,
    );
    let mut attributes = vec![(
        Checked::Valid(gltf::json::mesh::Semantic::Positions),
        positions,
    )];
    if let Some(normals) = primitive.normals.as_ref() {
        let mut bytes = Vec::with_capacity(normals.len() * 12);
        for normal in normals {
            for axis in normal {
                bytes.extend_from_slice(&axis.to_le_bytes());
            }
        }
        let accessor = layout.accessor(
            normals.len(),
            float(),
            Checked::Valid(AccessorType::Vec3),
            None,
            Some(Checked::Valid(gltf::json::buffer::Target::ArrayBuffer)),
            &bytes,
        );
        attributes.push((Checked::Valid(gltf::json::mesh::Semantic::Normals), accessor));
    }
    let indices = primitive.indices.as_ref().map(|values| {
        let width = indices_byte_width(values);
        let mut bytes = Vec::with_capacity(values.len() * width);
        for value in values {
            if width == 2 {
                bytes.extend_from_slice(&(*value as u16).to_le_bytes());
            } else {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        layout.accessor(
            values.len(),
            Checked::Valid(GenericComponentType(match width {
                2 => gltf::json::accessor::ComponentType::U16,
                _ => gltf::json::accessor::ComponentType::U32,
            })),
            Checked::Valid(AccessorType::Scalar),
            None,
            Some(Checked::Valid(gltf::json::buffer::Target::ElementArrayBuffer)),
            &bytes,
        )
    });
    Ok(gltf::json::mesh::Primitive {
        attributes: attributes.into_iter().collect(),
        indices,
        material: primitive
            .material
            .as_ref()
            .and_then(|name| materials.get(name.as_str()).copied()),
        mode: Checked::Valid(gltf::json::mesh::Mode::Triangles),
        targets: None,
        extensions: None,
        extras: Default::default(),
    })
}

fn bounds(values: impl Iterator<Item = f32>) -> (serde_json::Value, serde_json::Value) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut count = 0usize;
    for (index, value) in values.enumerate() {
        let axis = index % 3;
        min[axis] = min[axis].min(value);
        max[axis] = max[axis].max(value);
        count += 1;
    }
    if count == 0 {
        min = [0.0; 3];
        max = [0.0; 3];
    }
    (serde_json::json!(min), serde_json::json!(max))
}
