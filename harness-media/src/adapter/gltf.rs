//! # glTF/GLB adapter (plan M12, pure tier)
//!
//! ## Decision record (plan-mandated)
//!
//! - **Read** through the khronos-ecosystem `gltf` crate 1.4.1 (pure Rust,
//!   import-only BY DESIGN - digest-verified; `default-features = false`
//!   keeps its optional `image` codec dependency out of this tree). GLB
//!   containers additionally walk through this module's own offset-precise
//!   chunk parser ([`gltf_container`]) so truncations fail with the exact
//!   byte offset, not an opaque error.
//! - **Write** through `gltf-json`'s Serialize types + `gltf::Glb::to_vec`
//!   (binary.rs L177-231 in the crate source): NO maintained high-level
//!   glTF exporter exists, so the serde-subset writer IS the
//!   digest-endorsed shape. See [`gltf_write`] for the full record.
//! - **Compressed variants** (`KHR_draco_mesh_compression`,
//!   `EXT_meshopt_compression`) are refused with the typed
//!   [`MediaError::UnsupportedFeature`](crate::error::MediaError::UnsupportedFeature)
//!   - never mangled into a partial extraction.
//! - **Verification** is the M5 diff under [`tolerances`] (eps 1e-6;
//!   materials + name-keyed nodes unordered) PLUS the independent
//!   khronos-ecosystem re-parse ([`reparse`]) on every artifact. The
//!   plan's "Khronos glTF-Validator (pure Rust, in CI)" premise is
//!   factually wrong - the Khronos validator is DART, and the only
//!   crates.io wrapper downloads a prebuilt foreign binary at BUILD time
//!   and does not support Windows; neither is admissible in hermetic CI
//!   (standing constraint 5). The real Khronos validator therefore runs
//!   as the `#[ignore]` real-tool tier over every generated GLB
//!   (errors=0 pinned; see tests/gltf_roundtrip.rs).
//!
//! Licensing: glTF 2.0 (Khronos, ISO/IEC 12141:2022) is GREEN under the
//! Khronos RF IP framework - the cleanest licensing story in 3D.

use std::collections::BTreeSet;

use harness_core::event::ProjectionKind;
use serde_json::Value;

use crate::adapter::gltf_container::{GlbChunks, GlbFormatError};
use crate::adapter::{gltf_extract, gltf_spec, gltf_write};
use crate::diff::{verify_roundtrip, RoundTripVerdict, Tolerances};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;
use crate::projection::{
    ProjectionContext, ProjectionProvider, ProjectionRegistry, ProjectionWiringError,
};

/// Producer-disclosure engine name for every pure glTF dispatch.
pub const ENGINE: &str = "harness-media::gltf";

/// The M12 tolerance profile: float epsilon 1e-6; the materials array and
/// the name-keyed nodes array compare order-insensitively (the plan's only
/// sanctioned reordering escapes for this format).
#[must_use]
pub fn tolerances() -> Tolerances {
    Tolerances {
        float_epsilon: 1e-6,
        unordered_at: BTreeSet::from(["/materials".to_string(), "/nodes".to_string()]),
        ignored_at: BTreeSet::new(),
    }
}

/// Extract handler: `.gltf` and `.glb` bytes to the canonical tree + the
/// bounded scene outline, under caller-provided budgets.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    gltf_extract::extract(bytes, budgets)
}

/// Generate handler: the subset spec to `.gltf` or `.glb` bytes (the
/// spec's `container` field selects; `glb` is the default).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    gltf_write::generate(spec)
}

/// Verify handler: extract the produced bytes and diff against the
/// INDEPENDENT expected tree under the M12 profile; empty diff = PASS.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    verify_roundtrip(
        expected,
        bytes,
        |bytes| gltf_extract::extract(bytes, ExtractBudgets::default()),
        &tolerances(),
    )
}

/// The INDEPENDENT spec->tree projection (M5 provenance rule): the
/// expected tree a generated artifact must extract to, computed from the
/// spec alone. Fails typed on a spec that would not validate.
pub fn spec_tree(params: &Value) -> Result<Value, MediaError> {
    gltf_spec::spec_tree(params)
}

/// Independent re-parse through the khronos-ecosystem reader (full
/// validated load, both containers): breaks the writer/reader circularity
/// on every generated artifact. Returns the parsed asset version.
pub fn reparse(bytes: &[u8]) -> Result<String, MediaError> {
    ::gltf::Gltf::from_slice(bytes)
        .map(|gltf| gltf.document.as_json().asset.version.clone())
        .map_err(|_| MediaError::ParseFailure)
}

/// Walk a GLB container: JSON chunk + BIN chunk length, offset-precise
/// failures (surfaces as the typed `ParseFailure` with the offset).
pub fn glb_chunks(bytes: &[u8]) -> Result<GlbChunks<'_>, GlbFormatError> {
    crate::adapter::gltf_container::glb_chunks(bytes)
}

/// The deterministic glTF projector: a thin wrap over the extract path's
/// scene outline (text-only delivery is first-class).
pub struct GltfOutline;

impl ProjectionProvider for GltfOutline {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        gltf_extract::extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}

/// Register the deterministic projector alongside the adapter (one
/// format, one projector; contention is the typed wiring error).
pub fn register_projection(
    registry: &mut ProjectionRegistry,
) -> Result<(), ProjectionWiringError> {
    registry.register(FormatId::Gltf, std::sync::Arc::new(GltfOutline))
}
