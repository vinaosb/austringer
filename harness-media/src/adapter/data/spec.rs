//! The M22 tabular spec shared by the parquet and arrow-ipc lanes:
//! `{columns: [{name, type}], rows: [[values]]}` with a deliberately
//! small honest column-type surface (bool, int32, int64, double, utf8 -
//! anything else is the typed `InvalidSpec`, never a mangled write) and
//! hard bounds (`MAX_COLUMNS`, `MAX_ROWS`) so a spec can never ask the
//! writers for unbounded work.
//!
//! This module also owns the INDEPENDENT spec->tree projections (M5
//! provenance rule): the schema section, the per-column statistics, and
//! the normalized rows are computed from the SPEC's values - separate
//! code from either extractor, separately tested.

use serde_json::{Number, Value};

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Column cap: a wider spec is the typed `InvalidSpec`.
pub const MAX_COLUMNS: usize = 64;

/// Row cap: also the extract head-sample bound, so the verify gate
/// compares whole spec-driven writes (rows ordered-exact, M5/M22).
pub const MAX_ROWS: usize = 1000;

/// The honest column-type surface of the v0.2 data lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    /// Parquet BOOLEAN / Arrow Boolean.
    Bool,
    /// Parquet INT32 / Arrow Int32.
    Int32,
    /// Parquet INT64 / Arrow Int64.
    Int64,
    /// Parquet DOUBLE / Arrow Float64.
    Double,
    /// Parquet BYTE_ARRAY+UTF8 / Arrow Utf8.
    Utf8,
}

impl ColumnType {
    /// Parse the spec's type token; unknown tokens are `None`.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "bool" => Some(Self::Bool),
            "int32" => Some(Self::Int32),
            "int64" => Some(Self::Int64),
            "double" => Some(Self::Double),
            "utf8" => Some(Self::Utf8),
            _ => None,
        }
    }

    /// The canonical type name used in trees and projections.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Double => "double",
            Self::Utf8 => "utf8",
        }
    }

    /// The arrow DataType for this column (fields are nullable).
    #[must_use]
    pub fn arrow_type(self) -> arrow::datatypes::DataType {
        use arrow::datatypes::DataType;
        match self {
            Self::Bool => DataType::Boolean,
            Self::Int32 => DataType::Int32,
            Self::Int64 => DataType::Int64,
            Self::Double => DataType::Float64,
            Self::Utf8 => DataType::Utf8,
        }
    }

    /// Parse an arrow DataType back into the surface; anything outside
    /// the v0.2 surface is `None` (the caller refuses it typed).
    #[must_use]
    pub fn from_arrow_type(dtype: &arrow::datatypes::DataType) -> Option<Self> {
        use arrow::datatypes::DataType;
        match dtype {
            DataType::Boolean => Some(Self::Bool),
            DataType::Int32 => Some(Self::Int32),
            DataType::Int64 => Some(Self::Int64),
            DataType::Float64 => Some(Self::Double),
            DataType::Utf8 => Some(Self::Utf8),
            _ => None,
        }
    }

    fn accepts(self, value: &Value) -> bool {
        let typed = match (self, value) {
            (Self::Bool, Value::Bool(_)) => true,
            (Self::Int32, Value::Number(n)) => {
                n.as_i64().is_some_and(|v| i32::try_from(v).is_ok())
            }
            (Self::Int64, Value::Number(n)) => n.as_i64().is_some(),
            (Self::Double, Value::Number(_)) => true,
            (Self::Utf8, Value::String(_)) => true,
            _ => false,
        };
        typed || value.is_null()
    }

    /// Normalize a JSON cell into the canonical tree form for this type
    /// (doubles become f64 numbers; everything else passes through).
    #[must_use]
    pub fn canonical_cell(self, value: &Value) -> Value {
        if self == Self::Double
            && let Some(f) = value.as_f64()
        {
            return Number::from_f64(f).map_or(Value::Null, Value::Number);
        }
        value.clone()
    }
}

/// One named, typed column.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Column name (non-empty, unique within the spec).
    pub name: String,
    /// Column type.
    pub ctype: ColumnType,
}

/// The validated tabular spec shared by both data formats.
#[derive(Debug, Clone, PartialEq)]
pub struct TabularSpec {
    /// Ordered column list (order is the schema order).
    pub columns: Vec<Column>,
    /// Rows in spec order; each row is one value per column (`null` =
    /// SQL NULL - every column is nullable).
    pub rows: Vec<Vec<Value>>,
}

impl TabularSpec {
    /// Parse and validate a generation spec addressed to parquet or
    /// arrow-ipc. Strict: exactly the `columns`/`rows` keys, unique
    /// non-empty names within [`MAX_COLUMNS`], rows within [`MAX_ROWS`],
    /// ragged rows and wrong-typed cells are the typed `InvalidSpec`.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch or any contract
    /// violation above.
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if !matches!(spec.format, FormatId::Parquet | FormatId::Arrow) {
            return Err(MediaError::InvalidSpec);
        }
        let invalid = || MediaError::InvalidSpec;
        let object = spec.params.as_object().ok_or_else(invalid)?;
        if object.len() != 2 {
            return Err(invalid());
        }
        let columns_json = object["columns"].as_array().ok_or_else(invalid)?;
        if columns_json.is_empty() || columns_json.len() > MAX_COLUMNS {
            return Err(invalid());
        }
        let mut columns = Vec::new();
        for column in columns_json {
            let entry = column.as_object().ok_or_else(invalid)?;
            if entry.len() != 2 {
                return Err(invalid());
            }
            let name = entry["name"].as_str().ok_or_else(invalid)?;
            if name.is_empty() {
                return Err(invalid());
            }
            let token = entry["type"].as_str().ok_or_else(invalid)?;
            let ctype = ColumnType::parse(token).ok_or_else(invalid)?;
            if columns.iter().any(|existing: &Column| existing.name == name) {
                return Err(invalid());
            }
            columns.push(Column { name: name.to_owned(), ctype });
        }
        let rows_json = object["rows"].as_array().ok_or_else(invalid)?;
        if rows_json.len() > MAX_ROWS {
            return Err(invalid());
        }
        let width = columns.len();
        let mut rows = Vec::new();
        for row in rows_json {
            let cells = row.as_array().ok_or_else(invalid)?;
            if cells.len() != width {
                return Err(invalid());
            }
            for (cell, column) in cells.iter().zip(&columns) {
                if !column.ctype.accepts(cell) {
                    return Err(invalid());
                }
            }
            rows.push(
                cells
                    .iter()
                    .zip(&columns)
                    .map(|(cell, column)| column.ctype.canonical_cell(cell))
                    .collect(),
            );
        }
        Ok(Self { columns, rows })
    }

    /// The canonical `schema` tree section (shared by both formats).
    #[must_use]
    pub fn schema_json(&self) -> Value {
        Value::Array(
            self.columns
                .iter()
                .map(|column| serde_json::json!({"name": column.name, "type": column.ctype.name()}))
                .collect(),
        )
    }

    /// The canonical normalized rows (doubles as f64 numbers).
    #[must_use]
    pub fn rows_json(&self) -> Value {
        Value::Array(self.rows.iter().map(|row| Value::Array(row.clone())).collect())
    }

    /// The independent per-column statistics projection, computed from
    /// the spec's own values (M5 provenance: never from a written file).
    /// `{"min": v, "max": v, "nulls": n}`; an all-null column carries
    /// `nulls` only (min/max absent, mirroring parquet's has-min-max
    /// contract).
    #[must_use]
    pub fn column_stats(&self, index: usize) -> Value {
        let ctype = self.columns[index].ctype;
        let mut nulls = 0_u64;
        let mut min: Option<&Value> = None;
        let mut max: Option<&Value> = None;
        for row in &self.rows {
            let cell = &row[index];
            if cell.is_null() {
                nulls += 1;
            } else {
                if min.is_none_or(|current| cell_lt(cell, current)) {
                    min = Some(cell);
                }
                if max.is_none_or(|current| cell_lt(current, cell)) {
                    max = Some(cell);
                }
            }
        }
        let mut stats = serde_json::Map::new();
        if let (Some(min), Some(max)) = (min, max) {
            stats.insert("min".into(), ctype.canonical_cell(min));
            stats.insert("max".into(), ctype.canonical_cell(max));
        }
        stats.insert("nulls".into(), Value::from(nulls));
        Value::Object(stats)
    }
}

/// Value ordering per column semantics: exact i64 comparison for
/// integers, f64 for doubles, UTF-8 byte order for strings (parquet's
/// ByteArray law), `false < true` for bools.
fn cell_lt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_i64(), y.as_i64()) {
            (Some(x), Some(y)) => x < y,
            _ => x.as_f64().zip(y.as_f64()).is_some_and(|(x, y)| x < y),
        },
        (Value::String(x), Value::String(y)) => x.as_bytes() < y.as_bytes(),
        (Value::Bool(x), Value::Bool(y)) => !*x && *y,
        _ => false,
    }
}
