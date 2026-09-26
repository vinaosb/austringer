//! The STEP generation-spec schema (plan M20): the typed primitive
//! subset a model may drive - solids (box / cylinder / sphere with
//! dimensions, a placement transform, and a name), bounded to
//! [`MAX_SOLIDS`]. Parsed strictly from
//! [`GenerationSpec::params`](crate::generate::GenerationSpec);
//! violations are the typed `InvalidSpec`, never a guessed default.
//! BREP/PMI fidelity beyond this subset is explicitly NOT claimed (the
//! honesty row lives in the module docs).

use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Upper bound on solids one document may carry (bounded writer output
/// and bounded topology census).
pub const MAX_SOLIDS: usize = 128;
/// Upper bound on one solid name in bytes (bounded census keys).
const MAX_NAME_BYTES: usize = 64;
/// Upper bound on any dimension (metres) - bounded geometry, bounded
/// volumes.
const MAX_DIMENSION: f64 = 1e6;

/// The three spec-driven primitives (plan M20), each constructed via
/// `Part.makeBox` / `Part.makeCylinder` / `Part.makeSphere` inside
/// freecadcmd.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SolidShape {
    /// Axis-aligned box from the origin with the given extents.
    Box {
        /// Extents along local x/y/z.
        size: [f64; 3],
    },
    /// Cylinder of `radius` and `height` on the local +z axis.
    Cylinder {
        /// Radius.
        radius: f64,
        /// Height.
        height: f64,
    },
    /// Sphere centred on the local origin.
    Sphere {
        /// Radius.
        radius: f64,
    },
}

/// Placement transform (STEP placement semantics: column-vector compose
/// `T * Rz * Ry * Rx`, XYZ euler in radians).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    /// Translation.
    pub location: [f64; 3],
    /// XYZ euler rotation, radians.
    pub rotation_xyz: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            location: [0.0; 3],
            rotation_xyz: [0.0; 3],
        }
    }
}

/// One spec-driven solid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolidSpec {
    /// Solid name (unique, printable ASCII, non-empty, <= 64 bytes) -
    /// surfaces as the PRODUCT name and in the extract skeleton.
    pub name: String,
    /// The primitive to construct.
    pub shape: SolidShape,
    /// Placement transform.
    #[serde(default)]
    pub transform: Transform,
}

/// The STEP generation spec.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StepSpec {
    /// The solids to write, in order (spec-driven writes are
    /// ordered-exact).
    pub solids: Vec<SolidSpec>,
}

impl StepSpec {
    /// Parse and validate a generation spec addressed to step.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, non-finite numbers, non-positive or oversize dimensions,
    /// oversize/duplicate/non-printable names, or more than
    /// [`MAX_SOLIDS`] solids.
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Step {
            return Err(MediaError::InvalidSpec);
        }
        let parsed: Self =
            serde_json::from_value(spec.params.clone()).map_err(|_| MediaError::InvalidSpec)?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// The INDEPENDENT spec->tree projection (M5 provenance rule): the
    /// canonical topology-census tree this spec's document must verify
    /// to, computed WITHOUT running the tokenizer, the builder, or the
    /// mock's OCCT model.
    #[must_use]
    pub fn topology_tree(&self) -> serde_json::Value {
        super::tree::topology_tree(self)
    }

    fn validate(&self) -> Result<(), MediaError> {
        if self.solids.len() > MAX_SOLIDS {
            return Err(MediaError::InvalidSpec);
        }
        let mut names = std::collections::BTreeSet::new();
        for solid in &self.solids {
            if solid.name.is_empty()
                || solid.name.len() > MAX_NAME_BYTES
                || !solid.name.bytes().all(|byte| (0x20..0x7f).contains(&byte))
                || !names.insert(solid.name.clone())
            {
                return Err(MediaError::InvalidSpec);
            }
            let (dims, transform) = match solid.shape {
                SolidShape::Box { size } => (size.to_vec(), &solid.transform),
                SolidShape::Cylinder { radius, height } => {
                    (vec![radius, height], &solid.transform)
                }
                SolidShape::Sphere { radius } => (vec![radius], &solid.transform),
            };
            let finite = transform
                .location
                .iter()
                .chain(transform.rotation_xyz.iter())
                .all(|value| value.is_finite())
                && dims.iter().all(|value| value.is_finite());
            let in_range = dims
                .iter()
                .all(|value| value.is_sign_positive() && *value > 0.0 && *value <= MAX_DIMENSION);
            if !finite || !in_range {
                return Err(MediaError::InvalidSpec);
            }
        }
        Ok(())
    }
}
