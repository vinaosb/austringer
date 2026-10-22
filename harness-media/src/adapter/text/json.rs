//! # JSON/JSONL adapter, pure tier (plan M30)
//!
//! RFC 8259 via serde_json with the workspace's sorted-key stability
//! (the `preserve_order` feature is banned workspace-wide): reads
//! canonicalize to sorted-key compact form; the writer emits exactly
//! that. The JSONL lane (one value per non-empty line) auto-detects
//! the same way the M2 probe does: full single-value parse first,
//! line-parse second, both failing is the typed `ParseFailure`.
//!
//! ## Honesty rows
//!
//! - Duplicate keys take the last value (serde_json; RFC 8259 permits
//!   duplicates) - the canonical form is what round trips.
//! - Numbers survive exactly (serde_json preserves i64/u64/f64; the M5
//!   diff's integer/float meeting point applies).
//! - Nesting is bounded by serde_json's recursion cap - a pathological
//!   depth is the typed `ParseFailure`, never a stack overflow.
//! - The JSONL tree is the tagged `{"jsonl": true, "rows": [...]}`
//!   form; a single-value JSON document is the value itself.

use std::time::Instant;

use harness_core::event::ProjectionKind;
use serde_json::Value;

use crate::adapter::text::{tolerances, truncate_chars};
use crate::diff::{RoundTripVerdict, Tolerances, structural_diff};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;
use crate::projection::{ProjectionContext, ProjectionProvider};

/// Producer-disclosure engine name for every json op.
pub const ENGINE: &str = "harness-media::text::json";

/// The json text-projection bound: the M1 ceiling (2000 CHARS).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 2000;

const _: () = assert!(TEXT_PROJECTION_MAX_CHARS <= crate::extract::TEXT_PROJECTION_MAX_CHARS);

/// The json generation spec: the value lane (`{"value": v}`) or the
/// JSONL lane (`{"rows": [...]}`).
#[derive(Debug, Clone, PartialEq)]
pub struct JsonSpec {
    /// The document lane.
    pub lane: Lane,
}

/// The two json document lanes.
#[derive(Debug, Clone, PartialEq)]
pub enum Lane {
    /// A single JSON value (any value, scalar to tree).
    Value(Value),
    /// One value per line (>= 1 row).
    Rows(Vec<Value>),
}

impl JsonSpec {
    /// Parse and validate a generation spec addressed to json.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, a params object
    /// carrying neither/both of `value`/`rows`, a non-array `rows`, or
    /// an empty `rows` list.
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Json {
            return Err(MediaError::InvalidSpec);
        }
        let invalid = || MediaError::InvalidSpec;
        let object = spec.params.as_object().ok_or_else(invalid)?;
        match (object.get("value"), object.get("rows")) {
            (Some(value), None) => Ok(Self {
                lane: Lane::Value(value.clone()),
            }),
            (None, Some(Value::Array(rows))) if !rows.is_empty() => Ok(Self {
                lane: Lane::Rows(rows.clone()),
            }),
            _ => Err(invalid()),
        }
    }

    /// The INDEPENDENT spec->tree projection: the value itself, or the
    /// tagged jsonl tree.
    #[must_use]
    pub fn canonical_tree(&self) -> Value {
        match &self.lane {
            Lane::Value(value) => value.clone(),
            Lane::Rows(rows) => serde_json::json!({"jsonl": true, "rows": rows}),
        }
    }
}

/// Extract the canonical tree from json/jsonl bytes under
/// caller-provided safety budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] when the bytes are neither a single
/// JSON value nor a JSONL stream of them (non-utf8 included);
/// [`MediaError::ExtractTooLarge`] when the size pre-check or the
/// per-line budget checks trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = Instant::now();
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => {
            let head = serde_json::to_string(&value).unwrap_or_default();
            finish(&head, value)
        }
        Err(_) => extract_jsonl(bytes, budgets, started),
    }
}

fn extract_jsonl(
    bytes: &[u8],
    budgets: ExtractBudgets,
    started: Instant,
) -> Result<Extracted, MediaError> {
    let text = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    let mut rows = Vec::new();
    let mut collected = bytes.len() as u64;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if started.elapsed() >= budgets.deadline || collected > budgets.max_total_bytes {
            return Err(MediaError::ExtractTooLarge);
        }
        let value = serde_json::from_str::<Value>(trimmed).map_err(|_| MediaError::ParseFailure)?;
        collected += trimmed.len() as u64;
        rows.push(value);
    }
    if rows.is_empty() {
        return Err(MediaError::ParseFailure);
    }
    let head = serde_json::to_string(&rows[0]).unwrap_or_default();
    finish(
        &format!("jsonl: {} rows; first={head}", rows.len()),
        serde_json::json!({"jsonl": true, "rows": rows}),
    )
}

fn finish(projection_source: &str, tree: Value) -> Result<Extracted, MediaError> {
    let text_projection = truncate_chars(projection_source.to_owned(), TEXT_PROJECTION_MAX_CHARS);
    Ok(Extracted {
        tree,
        text_projection,
        thumbnails: Vec::new(),
    })
}

/// Generate deterministic canonical bytes from a validated spec:
/// sorted-key compact JSON (single trailing newline), or one compact
/// value per line for the JSONL lane.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec does not satisfy the json
/// contract (see [`JsonSpec::from_generation`]).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let parsed = JsonSpec::from_generation(spec)?;
    let bytes = match &parsed.lane {
        Lane::Value(value) => {
            let mut out = serde_json::to_vec(value).map_err(|_| MediaError::InvalidSpec)?;
            out.push(b'\n');
            out
        }
        Lane::Rows(rows) => {
            let mut out = Vec::new();
            for row in rows {
                out.extend_from_slice(
                    &serde_json::to_vec(row).map_err(|_| MediaError::InvalidSpec)?,
                );
                out.push(b'\n');
            }
            out
        }
    };
    Ok(bytes)
}

/// The json diff profile: float epsilon only; no unordered pointers,
/// no ignored subtrees.
#[must_use]
pub fn profile() -> Tolerances {
    tolerances()
}

/// Verify produced json bytes against the INDEPENDENT expected tree:
/// reparse (extract) then M5 structural diff.
///
/// # Errors
/// The extractor's typed failures;
/// [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let diff = structural_diff(expected, &extracted.tree, &profile())?;
    verdict(diff)
}

fn verdict(diff: crate::diff::StructuralDiff) -> Result<RoundTripVerdict, MediaError> {
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}

/// The json deterministic projector (M10 seam): a thin wrap over the
/// extractor's bounded projection.
pub struct JsonProjector;

impl ProjectionProvider for JsonProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}
