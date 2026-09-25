//! # .blend adapter, subprocess tier (plan M16 - the flagship 3D lane)
//!
//! Blender scenes through the M15 tool executor: probe stays the pure
//! M2 magic sniff (DNA parsing is deliberately NOT attempted in Rust -
//! digest 3.3: the .blend DNA struct catalogs shift per release and
//! hand-writing the binary is banned), while extract and generate run
//! headless Blender (`--background --factory-startup`, never user
//! prefs) over in-crate versioned python templates.
//!
//! ## Pipeline
//!
//! - [`extract`]: write the bytes + the dump script into the session
//!   temp, run `blender --background --factory-startup <file> --python
//!   dump_script.py`, parse the scene census JSON printed between the
//!   dump markers (robust to blender's own startup chatter), and render
//!   the bounded scene-outline projection.
//! - [`generate`]: render the builder script from the typed
//!   [`BlendSpec`] (the spec is embedded as a single-line JSON literal),
//!   run `blender --background --factory-startup --python build.py --
//!   <out.blend>`; the script constructs via `bpy.data` +
//!   `mesh.from_pydata` + `mesh.validate()` with `bpy.ops` reserved for
//!   the final `save_as_mainfile` (context/modal-dependent ops are
//!   banned by the plan).
//! - [`verify`]: extract(generate(bytes)) under the M5 diff with
//!   unordered-by-name objects and eps 1e-5.
//!
//! ## Hermetic triangulation
//!
//! Real Blender is opt-in and absent by default (typed
//! [`ToolUnavailable`](crate::error::MediaError::ToolUnavailable) with
//! the manifest install hint). The hermetic suite runs the mocktool
//! `blendbuild`/`blenddump` modes, whose census is DERIVED from the
//! builder script's embedded spec - so a round-trip cross-checks three
//! independent derivations: the python builder template, the mock's
//! Rust blender model, and this adapter's independent
//! spec->tree projection ([`BlendSpec::canonical_tree`]). The dump
//! stdout contract is pinned by a versioned schema test
//! ([`DUMP_SCHEMA`], mock-drift law).
//!
//! ## Honesty rows (surfaced verbatim by the M27 matrix)
//!
//! - Licensing (digest 3.3 BLEND row): YELLOW - no cross-version
//!   stability contract; never hand-write the binary; every read/write
//!   routes through the pinned tool lane.
//! - Version pin: the manifest pins the tested Blender line; the pin
//!   honestly reads `pending-first-real-run` until a real run happens
//!   (M16 waiver: no Blender on the dev machine at todo time - see the
//!   task evidence; the `#[ignore]` smoke test documents `BLENDER_BIN`).
//! - Byte-determinism: .blend is structurally reproducible, NOT
//!   byte-reproducible - verify is the re-open + census diff, never a
//!   hash (digest 3.3 references).
//! - Arbitrary real .blend files extract only with real Blender
//!   installed; the mock lane carries its own census fixtures.

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::extract::Extracted;
use crate::projection::{ProjectionContext, ProjectionProvider};
use crate::tool::ToolExecutor;

mod extract;
mod generate;
mod script;
mod spec;
mod tree;
mod verify;

pub use extract::extract;
pub use generate::generate;
pub use script::builder_script;
pub use spec::{BlendSpec, MaterialSpec, ObjectSpec, Primitive, Transform, MAX_OBJECTS};
pub use verify::{profile, verify};

/// The subprocess lane's tool (the M15 manifest key).
pub const TOOL: &str = "blender";
/// The versioned dump-script stdout contract (mock-drift law pin).
pub const DUMP_SCHEMA: &str = "harness-blend-dump/1";
/// The versioned builder-script embedded-spec schema.
pub const BUILD_SCHEMA: &str = "harness-blend-build/1";

/// The blend deterministic projector (M10 seam): a thin wrap over the
/// subprocess extractor's bounded scene outline. Real Blender required
/// (typed `ToolUnavailable` otherwise) - never a fabricated outline.
pub struct BlendProjector;

impl ProjectionProvider for BlendProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(
        &self,
        bytes: &[u8],
        context: &ProjectionContext<'_>,
    ) -> Result<String, MediaError> {
        let tools = ToolExecutor::default();
        let produced = extract(&tools, bytes, context.budgets)?;
        match produced.inner() {
            crate::executor::OpOutput::Extract(Extracted {
                text_projection, ..
            }) => Ok(text_projection.clone()),
            _ => Err(MediaError::NoProjector),
        }
    }
}

/// Registers [`BlendProjector`] for blend (one format, one projector).
/// The projection itself stays typed-refusing until a Blender-enabled
/// executor is bound at the delivery edge - never a fabricated outline.
///
/// # Errors
/// [`ProjectionWiringError::DuplicateProjector`] on contention.
pub fn register_projections(
    registry: &mut crate::projection::ProjectionRegistry,
) -> Result<(), crate::projection::ProjectionWiringError> {
    registry.register(crate::format::FormatId::Blend, std::sync::Arc::new(BlendProjector))
}
