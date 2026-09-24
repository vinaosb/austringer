//! # CSV adapter, pure tier (plan M30)
//!
//! RFC 4180 via the csv crate: schema (field count) + row-count
//! extract with the dialect/BOM hazards handled and disclosed, never
//! guessed. The comma dialect is the only one read (the digest's
//! "dialect chaos is the only hazard" note): semicolon/tab files parse
//! honestly as single-field comma-dialect rows; ragged rows refuse
//! typed (`flexible(false)`), never silently padded.
//!
//! ## Honesty rows
//!
//! - Reading is BOM-TOLERANT: a UTF-8 BOM is stripped and disclosed as
//!   `"bom": true` in the tree. The writer NEVER emits a BOM
//!   (write-BOM disclosed: we do not write one).
//! - Rows are counted uniformly - a header row is a semantic convention
//!   CSV cannot express, so `first_row` is reported, never guessed at.
//! - The writer is CRLF-terminated per RFC 4180 and quotes per the RFC
//!   (fields containing commas, quotes, or newlines are quoted).
//! - `field_count`/`first_row` come from the first record; empty input
//!   is a valid zero-row table.

use std::time::Instant;

use harness_core::event::ProjectionKind;
use serde_json::Value;

use crate::adapter::text::{tolerances, truncate_chars};
use crate::diff::{structural_diff, RoundTripVerdict, Tolerances};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;
use crate::projection::{ProjectionContext, ProjectionProvider};

/// Producer-disclosure engine name for every csv op.
pub const ENGINE: &str = "harness-media::text::csv";

/// The csv text-projection bound: the M1 ceiling (2000 CHARS).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 2000;

/// Upper bound on rows one generation may carry (bounded writer
/// output).
pub const MAX_ROWS: usize = 1_000_000;

const _: () = assert!(TEXT_PROJECTION_MAX_CHARS <= crate::extract::TEXT_PROJECTION_MAX_CHARS);

/// The csv generation spec: uniform-width rows (>= 1 row, >= 1 column);
/// the first row is the first record - headers are the caller's
/// semantic, not the format's.
#[derive(Debug, Clone, PartialEq)]
pub struct CsvSpec {
    /// Rows in document order.
    pub rows: Vec<Vec<String>>,
}

impl CsvSpec {
    /// Parse and validate a generation spec addressed to csv.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, an empty row list, ragged rows, a zero-width table, or
    /// more than [`MAX_ROWS`] rows.
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Csv {
            return Err(MediaError::InvalidSpec);
        }
        let invalid = || MediaError::InvalidSpec;
        #[derive(serde::Deserialize)]
        struct Params {
            rows: Vec<Vec<String>>,
        }
        let params: Params =
            serde_json::from_value(spec.params.clone()).map_err(|_| invalid())?;
        if params.rows.is_empty() || params.rows.len() > MAX_ROWS {
            return Err(invalid());
        }
        let width = params.rows[0].len();
        if width == 0 || params.rows.iter().any(|row| row.len() != width) {
            return Err(invalid());
        }
        Ok(Self { rows: params.rows })
    }

    /// The INDEPENDENT spec->tree projection: the schema tree the
    /// generated file must extract to (`bom` is always false - the
    /// writer never emits one).
    #[must_use]
    pub fn canonical_tree(&self) -> Value {
        serde_json::json!({
            "bom": false,
            "field_count": self.rows[0].len(),
            "first_row": self.rows[0],
            "row_count": self.rows.len(),
        })
    }
}

/// Extract the schema tree (BOM disclosure, field count, first row,
/// row count) from csv bytes under caller-provided safety budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-utf8 input, ragged rows, or a
/// malformed quoted field;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = Instant::now();
    let text = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    let (text, bom) = match text.strip_prefix('\u{feff}') {
        Some(rest) => (rest, true),
        None => (text, false),
    };
    let mut collected = bytes.len() as u64;
    if collected > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .from_reader(text.as_bytes());
    let mut row_count = 0_usize;
    let mut field_count = 0_usize;
    let mut first_row: Option<Value> = None;
    for record in reader.records() {
        if started.elapsed() >= budgets.deadline || collected > budgets.max_total_bytes {
            return Err(MediaError::ExtractTooLarge);
        }
        let record = record.map_err(|_| MediaError::ParseFailure)?;
        collected += record.as_slice().len() as u64;
        if row_count == 0 {
            field_count = record.len();
            first_row = Some(
                record.iter().map(str::to_owned).collect::<Vec<_>>().into(),
            );
        }
        row_count += 1;
    }
    let tree = serde_json::json!({
        "bom": bom,
        "field_count": field_count,
        "first_row": first_row,
        "row_count": row_count,
    });
    let first = match &tree["first_row"] {
        Value::Array(row) => row.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(","),
        _ => String::new(),
    };
    let text_projection = truncate_chars(
        format!("csv: {row_count} rows x {field_count} fields; first_row={first}"),
        TEXT_PROJECTION_MAX_CHARS,
    );
    Ok(Extracted { tree, text_projection, thumbnails: Vec::new() })
}

/// Generate deterministic RFC 4180 bytes (CRLF records, RFC quoting,
/// no BOM) from a validated spec.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec does not satisfy the csv
/// contract (see [`CsvSpec::from_generation`]).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let parsed = CsvSpec::from_generation(spec)?;
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    for row in &parsed.rows {
        writer.write_record(row).map_err(|_| MediaError::ParseFailure)?;
    }
    writer.into_inner().map_err(|_| MediaError::ParseFailure)
}

/// The csv diff profile: float epsilon only; no unordered pointers, no
/// ignored subtrees.
#[must_use]
pub fn profile() -> Tolerances {
    tolerances()
}

/// Verify produced csv bytes against the INDEPENDENT expected tree:
/// reparse (extract) then M5 structural diff.
///
/// # Errors
/// The extractor's typed failures;
/// [`MediaError::DiffTooLarge`] when the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let diff = structural_diff(expected, &extracted.tree, &profile())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}

/// The csv deterministic projector (M10 seam): a thin wrap over the
/// extractor's bounded projection.
pub struct CsvProjector;

impl ProjectionProvider for CsvProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(
        &self,
        bytes: &[u8],
        context: &ProjectionContext<'_>,
    ) -> Result<String, MediaError> {
        extract(bytes, context.budgets).map(|extracted| extracted.text_projection)
    }
}
