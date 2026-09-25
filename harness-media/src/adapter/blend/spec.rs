//! The blend generation-spec schema (plan M16): the typed primitive
//! subset a model may drive - objects (six primitives, name, transform,
//! optional Principled material), the world background, and the
//! collection layout. Parsed strictly from
//! [`GenerationSpec::params`](crate::generate::GenerationSpec);
//! violations are the typed `InvalidSpec`, never a guessed default.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::adapter::blend::tree;
use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Upper bound on objects one scene may carry (bounded writer output
/// and bounded dump).
pub const MAX_OBJECTS: usize = 256;
/// Upper bound on one object name in bytes (bounded dump keys).
const MAX_NAME_BYTES: usize = 64;

/// The six spec-driven primitives (plan M16), each constructed via
/// `mesh.from_pydata` at a canonical unit size (radius/ half-extent 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Primitive {
    /// Unit quad (4 verts / 4 edges / 1 face).
    Plane,
    /// Unit cube, half-extent 1 (8 / 12 / 6).
    Cube,
    /// UV sphere radius 1, 16 segments x 8 rings (114 / 240 / 128).
    Sphere,
    /// Cylinder radius 1 depth 2, 16 segments (32 / 48 / 18).
    Cylinder,
    /// Cone radius 1 depth 2, 16 segments (17 / 32 / 17).
    Cone,
    /// Icosphere radius 1, subdivision 1 (12 / 30 / 20).
    Icosphere,
}

/// TRS transform (Blender semantics: XYZ euler rotation in radians,
/// column-vector compose `T * Rz * Ry * Rx * S`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    /// Translation.
    pub location: [f64; 3],
    /// XYZ euler rotation, radians.
    pub rotation_euler: [f64; 3],
    /// Per-axis scale.
    pub scale: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            location: [0.0; 3],
            rotation_euler: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// A Principled-BSDF material slot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MaterialSpec {
    /// Linear base color, RGBA.
    pub base_color: [f64; 4],
    /// Metallic factor.
    pub metallic: f64,
    /// Roughness factor.
    pub roughness: f64,
}

/// One scene object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectSpec {
    /// Object name (unique, non-empty, <= [`MAX_NAME_BYTES`] bytes).
    pub name: String,
    /// The primitive to construct.
    pub primitive: Primitive,
    /// TRS transform.
    #[serde(default)]
    pub transform: Transform,
    /// Optional material (slot name is `<object>-material`).
    #[serde(default)]
    pub material: Option<MaterialSpec>,
}

/// The blend generation spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlendSpec {
    /// Scene objects.
    pub objects: Vec<ObjectSpec>,
    /// World background RGBA; `None` keeps Blender's factory default
    /// (0.05, 0.05, 0.05, 1.0).
    pub world_background: Option<[f64; 4]>,
    /// Collection the objects are linked into.
    pub collection: String,
}

impl Default for BlendSpec {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            world_background: None,
            collection: "Collection".to_owned(),
        }
    }
}

impl BlendSpec {
    /// Parse and validate a generation spec addressed to blend.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, non-finite numbers, out-of-range factors, oversize
    /// names, duplicate names, or more than [`MAX_OBJECTS`] objects.
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Blend {
            return Err(MediaError::InvalidSpec);
        }
        let parsed: Self =
            serde_json::from_value(spec.params.clone()).map_err(|_| MediaError::InvalidSpec)?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// The INDEPENDENT spec->tree projection (M5 provenance rule): the
    /// canonical dump-contract tree this spec's scene must extract to,
    /// computed WITHOUT running the extractor, the builder, or the
    /// mock's blender model.
    #[must_use]
    pub fn canonical_tree(&self) -> Value {
        tree::spec_tree(self)
    }

    fn validate(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        if self.objects.len() > MAX_OBJECTS
            || self.collection.is_empty()
            || self.collection.len() > MAX_NAME_BYTES
        {
            return Err(invalid());
        }
        let mut names = std::collections::BTreeSet::new();
        for object in &self.objects {
            if object.name.is_empty()
                || object.name.len() > MAX_NAME_BYTES
                || !names.insert(object.name.clone())
            {
                return Err(invalid());
            }
            let transform = &object.transform;
            let all_finite = transform
                .location
                .iter()
                .chain(transform.rotation_euler.iter())
                .chain(transform.scale.iter())
                .all(|value| value.is_finite());
            if !all_finite {
                return Err(invalid());
            }
            if let Some(material) = &object.material
                && !(material.base_color.iter().all(|value| value.is_finite())
                    && material.metallic.is_finite()
                    && material.roughness.is_finite()
                    && (0.0..=1.0).contains(&material.metallic)
                    && (0.0..=1.0).contains(&material.roughness)
                    && material.base_color.iter().all(|value| (0.0..=1.0).contains(value)))
            {
                return Err(invalid());
            }
        }
        if self
            .world_background
            .is_some_and(|background| background.iter().any(|value| !(0.0..=1.0).contains(value)))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
