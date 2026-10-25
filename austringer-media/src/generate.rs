//! Generation contract and the DETERMINISM CONTRACT every writer obeys.
//!
//! ## DETERMINISM CONTRACT
//!
//! Byte-determinism across runs is an adapter-contract property, not a
//! nicety: every format writer pins every document timestamp it emits
//! (OOXML `docProps` created/modified, `gpkg_contents.last_change`, PDF
//! `CreationDate`, STEP `FILE_NAME` clock, and every sibling field) to the
//! fixed epoch [`DETERMINISM_EPOCH`] (2026-01-01T00:00:00Z) unless the
//! generation spec explicitly overrides the value. The M25 round-trip
//! evals rely on this: byte-identical repeat generations are proof of
//! determinism, and any byte drift must trace to a spec override, never to
//! ambient clock state.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::format::FormatId;

/// The fixed timestamp every writer pins document clocks to unless the
/// spec overrides: `2026-01-01T00:00:00Z`.
pub const DETERMINISM_EPOCH: &str = "2026-01-01T00:00:00Z";

/// The same instant as Unix seconds, for writers that embed numeric
/// timestamps (GeoPackage `last_change`, Parquet metadata).
pub const DETERMINISM_EPOCH_UNIX: i64 = 1_767_225_600;

/// Spec-driven generation input: target format plus free-form parameters.
///
/// `params` is validated by the format's adapter against its contract;
/// validation failures are the typed [`crate::error::MediaError::InvalidSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationSpec {
    /// The format to generate.
    pub format: FormatId,
    /// Format-specific parameters (schema owned by the adapter).
    pub params: Value,
}
