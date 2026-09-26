//! # GeoTIFF/COG adapter (plan M21 - read pure, write via GDAL)
//!
//! Extract is the PURE lane: the image-rs `tiff` crate walks the IFD
//! tags and the GeoKey directory (tags 34735/34736/34737, OGC 19-075)
//! for the model type, the keyable EPSG (citation fallback; "unknown
//! CRS" honest otherwise), the pixel scale + tiepoint affine bounds IN
//! THE FILE'S CRS, bands/dtype/dims, and a bounded head-pixel sample;
//! the COG profile facts (tiled + overview count + IFD ordering) are
//! reported, never re-written. A plain TIFF without geo keys extracts
//! its structure with the honest "no GeoKey directory" note - never a
//! mislabeled GeoTIFF.
//!
//! Generation is the SUBPROCESS lane: pure-Rust GeoTIFF writing is
//! manual-GeoKeys-painful and COG generation is `gdal_translate -of
//! COG` (digest 3.6), so the adapter writes the pixel baseline (plain
//! uncompressed u8 tiff) plus a staged spec contract, and
//! `gdal_translate` attaches the georeferencing (`-a_srs EPSG:...`,
//! `-a_ullr`, `-ot Byte -b 1`, `-of COG` when the spec flags it).
//! Verify runs `gdalinfo -json` (size / band type / geoTransform /
//! LAYOUT=COG assert) and diffs the pure re-extract against the
//! INDEPENDENT spec->tree projection under [`profile`].
//!
//! ## Honesty rows (surfaced verbatim by the M27 matrix)
//!
//! - NO REPROJECTION: bounds are reported in the file's CRS with the
//!   EPSG disclosed; PROJ axis-order normalization is the #1
//!   geospatial bug class and is deliberately NOT attempted (digest
//!   4).
//! - Licensing (digest 3.6): TIFF 6.0 + OGC GeoTIFF 1.1 GREEN (LZW
//!   patent expired 2004); COG = Radiant Earth community spec, GREEN.
//! - Determinism: TIFF carries no clock fields; the baseline writer
//!   and the mock tool lane are byte-deterministic (nothing to pin to
//!   the M1 epoch).
//! - COG profile recognition is the tiled + overviews + IFD-ordering
//!   signature; the AUTHORITATIVE cloud-optimized claim is the
//!   gdalinfo LAYOUT assert in verify (the real-tool tier / M25
//!   sweep), not this extractor's heuristic.

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};
use crate::tool::ToolExecutor;

mod extract;
mod generate;
mod spec;
mod verify;

pub use extract::extract;
pub use generate::generate;
pub use spec::{GeoRasterSpec, GeoTransform};
pub use verify::{profile, verify};

/// The subprocess lane's tool (the M15 manifest key).
pub const TOOL: &str = "gdal";
/// The pure extract engine id (producer disclosure).
pub const ENGINE: &str = "harness-media::geotiff";

/// The georaster deterministic projector (M10 seam): a thin wrap over
/// the PURE extractor's bounded projection (no tool needed - unlike
/// the blend lane, geotiff extract never touches the subprocess).
pub struct GeoRasterProjector;

impl ProjectionProvider for GeoRasterProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(
        &self,
        bytes: &[u8],
        context: &ProjectionContext<'_>,
    ) -> Result<String, MediaError> {
        let extracted = extract::extract(bytes, context.budgets)?;
        Ok(extracted.text_projection)
    }
}

/// Registers [`GeoRasterProjector`] for geotiff (one format, one
/// projector).
///
/// # Errors
/// [`ProjectionWiringError::DuplicateProjector`] on contention.
pub fn register_projections(
    registry: &mut ProjectionRegistry,
) -> Result<(), crate::projection::ProjectionWiringError> {
    registry.register(crate::format::FormatId::Geotiff, std::sync::Arc::new(GeoRasterProjector))
}

/// The plan's default-disabled gate shared by generate and verify:
/// refuse typed before any staging when gdal is not opted in and
/// installed (probe/extract stay pure).
pub(super) fn unavailable(tools: &ToolExecutor) -> Result<(), MediaError> {
    match tools.availability(TOOL) {
        crate::tool::ToolAvailability::Available { .. } => Ok(()),
        crate::tool::ToolAvailability::Unavailable { install_hint } => {
            Err(MediaError::ToolUnavailable {
                tool: TOOL.to_owned(),
                install_hint,
            })
        }
    }
}
