//! Extraction contract: canonical tree, bounded text projection,
//! thumbnails, and the caller-provided safety budgets.
//!
//! ## Extract safety (contract, enforcement lands with the extractors)
//!
//! Pure extract loops honor a wall-clock deadline plus a total-size cap,
//! both supplied by the caller as [`ExtractBudgets`]. When either trips the
//! extractor fails with the typed [`crate::error::MediaError::ExtractTooLarge`] - it
//! never hangs and never allocates unbounded. Budgets are fixture-scoped:
//! callers size them to the fixture at hand (a 10k-page PDF or a 500 MB
//! parquet file must fail typed, quickly), and per-format adapters may
//! lower the projection default below the hard ceiling.

use std::time::Duration;

use harness_core::event::MediaRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Hard ceiling for a text projection, in CHARS (never "tokens").
/// Per-format defaults may be lower; nothing may exceed this.
pub const TEXT_PROJECTION_MAX_CHARS: usize = 2000;

/// Default wall-clock extract deadline (fixture-scoped starting point;
/// callers narrow or raise it per fixture).
pub const DEFAULT_EXTRACT_DEADLINE: Duration = Duration::from_secs(10);

/// Default cumulative byte cap on what one extractor may materialize
/// (bytes read plus tree/projection output). Mirrors the 64 MiB source
/// ceiling precedent in harness-ast's parse gate.
pub const DEFAULT_EXTRACT_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

/// The result of a successful extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extracted {
    /// Canonical structural form of the document: a serde_json tree whose
    /// object keys serialize sorted (BTreeMap ordering; the
    /// `preserve_order` feature must never be enabled). This is the form
    /// the M5 structural diff compares.
    pub tree: Value,
    /// Deterministic bounded text rendering of the document, usable by any
    /// model (text-only delivery is first-class). Bound:
    /// [`TEXT_PROJECTION_MAX_CHARS`] CHARS; per-format defaults may be
    /// lower.
    pub text_projection: String,
    /// Content-addressed thumbnails of the source (usually zero or one).
    pub thumbnails: Vec<MediaRef>,
}

/// Caller-provided extract safety budget: wall-clock deadline + total-size
/// cap. Enforcement lands with the extractors (M11 onward); passing a
/// budget is part of the extract contract from day one, and tripping it is
/// always the typed [`crate::error::MediaError::ExtractTooLarge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractBudgets {
    /// Wall-clock deadline for the whole extract loop.
    pub deadline: Duration,
    /// Cumulative byte cap on everything the extractor materializes
    /// (input bytes read plus tree/projection output built).
    pub max_total_bytes: u64,
}

impl Default for ExtractBudgets {
    fn default() -> Self {
        Self {
            deadline: DEFAULT_EXTRACT_DEADLINE,
            max_total_bytes: DEFAULT_EXTRACT_MAX_TOTAL_BYTES,
        }
    }
}
