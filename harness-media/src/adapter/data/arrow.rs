//! # Arrow IPC adapter, pure tier (plan M22)
//!
//! The Arrow IPC FILE format (encapsulated messages between `ARROW1`
//! magics; MIME `application/x-arrow-ipc`) via the same arrow-rs
//! crate's ipc module. Extract = arrow Schema + record-batch census +
//! bounded head sample; generate = the tabular spec as one record
//! batch; verify = re-read with an arrow `Schema::eq` check FOLDED
//! into the M5 structural diff (a schema mismatch surfaces as a
//! changed `/schema` node - never a silent pass).

use std::io::Cursor;

use arrow::datatypes::{Field, Schema};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use serde_json::Value;

use super::spec::{ColumnType, TabularSpec};
use super::{budget_tripped, cell_value, record_batch, tolerances, truncate_chars};
use crate::diff::{structural_diff, Change, RoundTripVerdict};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted, TEXT_PROJECTION_MAX_CHARS};
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Producer-disclosure engine name for every arrow-ipc op.
pub const ENGINE: &str = "harness-media::data::arrow";

/// The INDEPENDENT spec->tree projection (M5 provenance rule): schema,
/// the single record batch's census, and the normalized rows.
#[must_use]
pub fn expected_tree(spec: &TabularSpec) -> Value {
    serde_json::json!({
        "format": "arrow",
        "num_rows": spec.rows.len(),
        "schema": spec.schema_json(),
        "batches": [{ "rows": spec.rows.len(), "columns": spec.columns.len() }],
        "rows": spec.rows_json(),
    })
}

/// Extract the canonical census tree from Arrow IPC file bytes:
/// schema, per-batch row/column census, bounded head sample.
///
/// # Errors
/// [`MediaError::ParseFailure`] on truncated/corrupt messages;
/// [`MediaError::UnsupportedFeature`] `column-type:<T>` outside the
/// v0.2 surface; [`MediaError::ExtractTooLarge`] when budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = std::time::Instant::now();
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let mut reader =
        FileReader::try_new(Cursor::new(bytes), None).map_err(|_| MediaError::ParseFailure)?;
    let schema = reader.schema();
    let ctypes = schema
        .fields()
        .iter()
        .map(|field| {
            ColumnType::from_arrow_type(field.data_type()).ok_or_else(|| {
                MediaError::UnsupportedFeature {
                    format: FormatId::Arrow,
                    feature: format!("column-type:{}", field.data_type()),
                    reason: "the v0.2 data lane covers bool/int32/int64/double/utf8 columns"
                        .into(),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut batches = Vec::new();
    let mut rows = Vec::new();
    let mut num_rows = 0_u64;
    let mut collected = bytes.len() as u64;
    for batch in &mut reader {
        if let Some(trip) = budget_tripped(started, budgets, collected) {
            return Err(trip);
        }
        let batch = batch.map_err(|_| MediaError::ParseFailure)?;
        batches.push(serde_json::json!({
            "rows": batch.num_rows(),
            "columns": batch.num_columns(),
        }));
        num_rows += batch.num_rows() as u64;
        for row in 0..batch.num_rows() {
            if rows.len() >= super::MAX_ROWS {
                break;
            }
            let mut values = Vec::with_capacity(ctypes.len());
            for (index, column) in batch.columns().iter().enumerate() {
                values.push(cell_value(column.as_ref(), row, ctypes[index])?);
            }
            collected += values.len() as u64 * 8;
            rows.push(Value::Array(values));
        }
    }
    let tree = serde_json::json!({
        "format": "arrow",
        "num_rows": num_rows,
        "schema": schema.fields().iter().zip(&ctypes).map(|(field, ctype)| serde_json::json!({
            "name": field.name(), "type": ctype.name()
        })).collect::<Vec<_>>(),
        "batches": batches,
        "rows": rows,
    });
    let first_row = tree["rows"]
        .as_array()
        .and_then(|rows| rows.first().cloned())
        .unwrap_or(Value::Null);
    let projection = truncate_chars(
        format!(
            "arrow: {num_rows} rows, {} batch(es); columns: {}; first row: {first_row}",
            tree["batches"].as_array().map_or(0, std::vec::Vec::len),
            tree["schema"],
        ),
        TEXT_PROJECTION_MAX_CHARS,
    );
    Ok(Extracted { tree, text_projection: projection, thumbnails: Vec::new() })
}

/// Generate Arrow IPC file bytes from the tabular spec: one record
/// batch, default IPC options (deterministic alignment; no clock in
/// the footer, so byte-determinism holds by construction).
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec fails the tabular contract.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    if spec.format != FormatId::Arrow {
        return Err(MediaError::InvalidSpec);
    }
    let parsed = TabularSpec::from_generation(spec)?;
    let batch = record_batch(&parsed)?;
    let mut writer = FileWriter::try_new(Vec::new(), &batch.schema())
        .map_err(|_| MediaError::InvalidSpec)?;
    writer.write(&batch).map_err(|_| MediaError::InvalidSpec)?;
    writer.into_inner().map_err(|_| MediaError::InvalidSpec)
}

/// Verify produced arrow bytes against the INDEPENDENT expected tree:
/// re-read (extract), diff the canonical trees, AND fold an arrow
/// `Schema::eq` check in (a mismatch reports as a changed `/schema`
/// node even when the projected schema JSON would coincidentally
/// match).
///
/// # Errors
/// The extractor's typed failures; [`MediaError::DiffTooLarge`] when
/// the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let mut diff = structural_diff(expected, &extracted.tree, &tolerances())?;
    let expected_schema = schema_from_tree(expected);
    let produced_schema = FileReader::try_new(Cursor::new(bytes), None)
        .ok()
        .map(|reader| reader.schema());
    if let (Some(expected_schema), Some(produced_schema)) = (expected_schema, produced_schema)
        && expected_schema != *produced_schema
    {
        diff.changed.push(Change {
            path: "/schema".into(),
            expected: expected["schema"].clone(),
            actual: extracted.tree["schema"].clone(),
        });
    }
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}

/// Rebuild the arrow Schema from an expected tree's schema section
/// (None when the section is not the canonical shape - the structural
/// diff then carries the failure).
fn schema_from_tree(expected: &Value) -> Option<Schema> {
    let mut fields = Vec::new();
    for column in expected.get("schema")?.as_array()? {
        let name = column.get("name")?.as_str()?;
        let token = column.get("type")?.as_str()?;
        let ctype = ColumnType::parse(token)?;
        fields.push(Field::new(name, ctype.arrow_type(), true));
    }
    if fields.is_empty() {
        return None;
    }
    Some(Schema::new(fields))
}
