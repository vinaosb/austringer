//! # STEP adapter (plan M20 - pure extract + freecadcmd generation)
//!
//! ISO 10303-21 clear-text STEP: the census surface is extracted by a
//! hand-rolled pure-Rust tokenizer (header AP name, entity census by
//! TYPE, the PRODUCT chain skeleton, units, CARTESIAN_POINT bbox),
//! while GENERATION runs through the opt-in freecadcmd subprocess (the
//! digest's verdict: general-purpose PMI-complete STEP generation is
//! subprocess-only - `brepkit-io` is AGPL = banned, `ruststep` is
//! deserialize-only/dormant, and pure-Rust write exists solely
//! kernel-scoped). The spec's honest subset is box/cylinder/sphere
//! solids with a placement transform - BREP/PMI fidelity beyond that
//! is explicitly NOT claimed.
//!
//! ## Pipeline
//!
//! - [`extract`]: PURE - the tokenizer scans the exchange structure
//!   under caller budgets (records/types/products caps + wall clock);
//!   never touches the tool lane.
//! - [`generate`]: render the builder script from the typed
//!   [`StepSpec`] (single-line embedded JSON), run `freecadcmd
//!   build.py <out>`; the script constructs via `Part.makeBox` /
//!   `makeCylinder` / `makeSphere` + Placement, exports the
//!   AP214-flavored STEP, then pins the FILE_NAME clock to the M1
//!   determinism epoch.
//! - [`verify`]: re-read the produced bytes through the topology-dump
//!   script (`freecadcmd dump.py <in>`) and diff the census
//!   (faces/edges/vertices/volume/bbox) against the INDEPENDENT
//!   spec->tree projection. The FILE_NAME stamp never enters the
//!   census - OCCT's wall-clock stamping cannot break the diff.
//!
//! ## Hermetic triangulation
//!
//! Real freecadcmd is opt-in and absent by default (typed
//! [`ToolUnavailable`](crate::error::MediaError::ToolUnavailable) with
//! the manifest install hint). The hermetic suite runs the mocktool
//! `stepbuild`/`stepdump` modes, whose census is DERIVED from the
//! builder script's embedded spec - so a round-trip cross-checks three
//! independent derivations: the python builder template, the mock's
//! Rust OCCT model, and this adapter's independent spec->tree
//! projection. The dump stdout contract is pinned by a versioned
//! schema test ([`DUMP_SCHEMA`], mock-drift law).
//!
//! ## Honesty rows (surfaced verbatim by the M27 matrix)
//!
//! - Licensing (digest 3.3 STEP row): YELLOW - spec text ISO-paywalled
//!   (usage unrestricted; free EXPRESS schema mirrors); every MCAD
//!   package speaks it.
//! - Version pin: the manifest pins the tested FreeCAD line; the pin
//!   honestly reads `pending-first-real-run` until a real run happens
//!   (M20 waiver: no FreeCAD on the dev machine at todo time - see the
//!   task evidence; the `#[ignore]` smoke test documents `FREECAD_BIN`).
//! - Topology table: the per-primitive face/edge/vertex counts are the
//!   Euler-consistent textbook pins; OCCT's own bookkeeping replaces
//!   them at the first real capture.
//! - Byte-determinism: FILE_NAME is pinned to the fixed epoch, but
//!   verify is the re-read + census diff, never a hash (OCCT output is
//!   not byte-stable by design).

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::projection::{ProjectionContext, ProjectionProvider};

mod extract;
mod generate;
mod interpret;
mod script;
mod spec;
mod tokenizer;
mod tree;
mod verify;

pub use extract::extract;
pub use generate::generate;
pub use script::builder_script;
pub use spec::{MAX_SOLIDS, SolidShape, SolidSpec, StepSpec, Transform};
pub use verify::{profile, read_topology_census, verify};

/// The pure tokenizer engine name (producer disclosure).
pub const ENGINE: &str = "harness-media::step";
/// The subprocess lane's tool (the M15 manifest key).
pub const TOOL: &str = "freecadcmd";
/// The versioned extract-tree schema (mock-drift law pin).
pub const EXTRACT_SCHEMA: &str = "harness-step-extract/1";
/// The versioned topology-census stdout contract (mock-drift law pin).
pub const DUMP_SCHEMA: &str = "harness-step-topology/1";
/// The versioned builder-script embedded-spec schema.
pub const BUILD_SCHEMA: &str = "harness-step-build/1";

/// The step deterministic projector (M10 seam): a thin wrap over the
/// pure tokenizer's bounded structure summary - no tool required.
pub struct StepProjector;

impl ProjectionProvider for StepProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}

/// Registers [`StepProjector`] for step (one format, one projector).
///
/// # Errors
/// [`ProjectionWiringError::DuplicateProjector`](crate::projection::ProjectionWiringError)
/// on contention.
pub fn register_projections(
    registry: &mut crate::projection::ProjectionRegistry,
) -> Result<(), crate::projection::ProjectionWiringError> {
    registry.register(
        crate::format::FormatId::Step,
        std::sync::Arc::new(StepProjector),
    )
}
