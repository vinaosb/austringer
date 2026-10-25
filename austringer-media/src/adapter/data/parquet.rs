//! # Parquet adapter, pure tier (plan M22)
//!
//! arrow-rs `parquet` crate (reference implementation; Apache-2.0;
//! digest section 3.5 GREEN - `PAR1` magic, MIME
//! `application/vnd.apache.parquet`). Extract = schema + row-group
//! census + per-column statistics (min/max/nulls where present) + a
//! bounded head-`MAX_ROWS` sample; generate = the tabular spec written
//! as ONE row group, UNCOMPRESSED, chunk statistics on, no
//! arrow-specific metadata (a canonical parquet file any engine reads);
//! verify = the M5 structural diff against the INDEPENDENT
//! [`expected_tree`] projection - rows ordered-exact, schema exact,
//! stats exact (single-row-group spec-driven writes, M5/M22 aligned).

use bytes::Bytes;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::arrow_writer::ArrowWriterOptions;
use parquet::basic::Compression;
use parquet::file::properties::{EnabledStatistics, WriterProperties, WriterVersion};
use serde_json::{Map, Value};

use super::spec::{ColumnType, TabularSpec};
use super::{budget_tripped, cell_value, parquet_census, record_batch, tolerances};
use crate::diff::{RoundTripVerdict, structural_diff};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Producer-disclosure engine name for every parquet op.
pub const ENGINE: &str = "austringer-media::data::parquet";

/// The INDEPENDENT spec->tree projection (M5 provenance rule): schema,
/// the single row group's per-column statistics (computed from the
/// spec's own values), and the normalized rows. A zero-row spec writes
/// no row group (arrow-rs drops empty row groups) - mirrored here.
#[must_use]
pub fn expected_tree(spec: &TabularSpec) -> Value {
    let row_groups = if spec.rows.is_empty() {
        Value::Array(Vec::new())
    } else {
        let columns: Vec<Value> = spec
            .columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let mut entry = Map::new();
                entry.insert("name".into(), Value::String(column.name.clone()));
                entry.insert("stats".into(), spec.column_stats(index));
                Value::Object(entry)
            })
            .collect();
        serde_json::json!([{ "num_rows": spec.rows.len(), "columns": columns }])
    };
    serde_json::json!({
        "format": "parquet",
        "num_rows": spec.rows.len(),
        "row_groups": row_groups,
        "schema": spec.schema_json(),
        "rows": spec.rows_json(),
    })
}

/// Extract the canonical census tree: schema, row-group count +
/// per-column statistics, and a bounded head sample of rows.
///
/// # Errors
/// [`MediaError::ParseFailure`] on a truncated footer or corrupt pages;
/// [`MediaError::UnsupportedFeature`] `compression:<CODEC>` for a
/// C-backed codec (LZ4/ZSTD - never half-decoded) or
/// `column-type:<T>` outside the v0.2 surface;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = std::time::Instant::now();
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let builder = ParquetRecordBatchReaderBuilder::try_new(Bytes::copy_from_slice(bytes))
        .map_err(|_| MediaError::ParseFailure)?;
    for group in builder.metadata().row_groups() {
        for chunk in group.columns() {
            let codec = chunk.compression();
            if !matches!(
                codec,
                Compression::UNCOMPRESSED
                    | Compression::SNAPPY
                    | Compression::GZIP(_)
                    | Compression::BROTLI(_)
            ) {
                return Err(MediaError::UnsupportedFeature {
                    format: FormatId::Parquet,
                    feature: format!("compression:{codec}"),
                    reason: "only the pure-Rust codecs (uncompressed/snappy/gzip/brotli) are \
                        compiled in; the C-backed codecs are deliberately absent from this tree"
                        .into(),
                });
            }
        }
    }
    let schema = builder.schema().clone();
    let ctypes = schema
        .fields()
        .iter()
        .map(|field| {
            ColumnType::from_arrow_type(field.data_type()).ok_or_else(|| {
                MediaError::UnsupportedFeature {
                    format: FormatId::Parquet,
                    feature: format!("column-type:{}", field.data_type()),
                    reason: "the v0.2 data lane covers bool/int32/int64/double/utf8 columns".into(),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let row_groups = parquet_census::row_groups_json(builder.metadata(), &schema);
    let num_rows = builder.metadata().file_metadata().num_rows();
    let reader = builder
        .with_batch_size(super::MAX_ROWS)
        .build()
        .map_err(|_| MediaError::ParseFailure)?;
    let mut rows = Vec::new();
    let mut collected = bytes.len() as u64;
    for batch in reader {
        if let Some(trip) = budget_tripped(started, budgets, collected) {
            return Err(trip);
        }
        let batch = batch.map_err(|_| MediaError::ParseFailure)?;
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
        if rows.len() >= super::MAX_ROWS {
            break;
        }
    }
    let tree = parquet_census::census_tree(&schema, &ctypes, num_rows, row_groups, rows);
    Ok(Extracted {
        text_projection: parquet_census::projection_text(&tree),
        tree,
        thumbnails: Vec::new(),
    })
}

/// Generate a canonical parquet file from the tabular spec: one row
/// group, UNCOMPRESSED pages, chunk statistics, no arrow-specific
/// metadata. No clock is written (parquet FileMetaData has none), so
/// byte-determinism holds by construction.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec fails the tabular contract.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    if spec.format != FormatId::Parquet {
        return Err(MediaError::InvalidSpec);
    }
    let parsed = TabularSpec::from_generation(spec)?;
    let batch = record_batch(&parsed)?;
    let props = WriterProperties::builder()
        .set_writer_version(WriterVersion::PARQUET_1_0)
        .set_compression(Compression::UNCOMPRESSED)
        .set_statistics_enabled(EnabledStatistics::Chunk)
        .set_max_row_group_row_count(Some(super::MAX_ROWS + 1))
        .build();
    let options = ArrowWriterOptions::new()
        .with_properties(props)
        .with_skip_arrow_metadata(true);
    let mut writer = ArrowWriter::try_new_with_options(Vec::new(), batch.schema(), options)
        .map_err(|_| MediaError::InvalidSpec)?;
    writer.write(&batch).map_err(|_| MediaError::InvalidSpec)?;
    writer.into_inner().map_err(|_| MediaError::InvalidSpec)
}

/// Verify produced parquet bytes against the INDEPENDENT expected tree:
/// extract, then the M5 structural diff (schema exact, rows
/// ordered-exact, statistics exact; float epsilon for f64 print
/// cycles).
///
/// # Errors
/// The extractor's typed failures; [`MediaError::DiffTooLarge`] when
/// the diff caps trip.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let diff = structural_diff(expected, &extracted.tree, &tolerances())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}
