//! # TOML adapter, pure tier (plan M30)
//!
//! TOML 1.0 via the toml crate: parse-tree extract (normalized to
//! JSON, datetimes as their TOML display strings) and a canonical
//! writer. Generation accepts JSON-shaped tables and refuses - typed,
//! never mangled - what TOML cannot represent: a non-table root,
//! `null`, and heterogeneous arrays.
//!
//! ## Honesty rows
//!
//! - Non-finite TOML floats normalize to JSON `null` (the M5 pinned
//!   decision: canonical trees never carry non-finite numbers).
//! - Generated documents serialize datetimes as STRINGS (JSON input
//!   cannot carry a native TOML datetime) - an honest one-way
//!   normalization, disclosed.
//! - The toml crate's serializer emits scalar keys first, then
//!   tables/arrays-of-tables, keys sorted within each group -
//!   deterministic and round-trip-stable.

use std::time::Instant;

use harness_core::event::ProjectionKind;
use serde_json::{Map, Number, Value};

use crate::adapter::text::{tolerances, truncate_chars};
use crate::diff::{structural_diff, RoundTripVerdict, Tolerances};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};
use crate::format::FormatId;
use crate::generate::GenerationSpec;
use crate::projection::{ProjectionContext, ProjectionProvider};

/// Producer-disclosure engine name for every toml op.
pub const ENGINE: &str = "harness-media::text::toml";

/// The toml text-projection bound: the M1 ceiling (2000 CHARS).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 2000;

const _: () = assert!(TEXT_PROJECTION_MAX_CHARS <= crate::extract::TEXT_PROJECTION_MAX_CHARS);

/// The toml generation spec: a JSON object that must be representable
/// as a TOML table.
#[derive(Debug, Clone, PartialEq)]
pub struct TomlSpec {
    /// The document table.
    pub value: Value,
}

impl TomlSpec {
    /// Parse and validate a generation spec addressed to toml.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, a non-object root, a `null` anywhere, or a
    /// heterogeneous array (TOML 1.0 requires same-type array
    /// elements).
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Toml {
            return Err(MediaError::InvalidSpec);
        }
        let object = spec.params.as_object().ok_or(MediaError::InvalidSpec)?;
        let value = object
            .get("value")
            .ok_or(MediaError::InvalidSpec)?
            .clone();
        if !value.is_object() || !representable(&value) {
            return Err(MediaError::InvalidSpec);
        }
        Ok(Self { value })
    }

    /// The INDEPENDENT spec->tree projection: the JSON value itself
    /// (exactly what a conforming document parses back to).
    #[must_use]
    pub fn canonical_tree(&self) -> Value {
        self.value.clone()
    }
}

fn representable(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(_) | Value::Number(_) | Value::String(_) => true,
        Value::Array(items) => {
            let mut kind: Option<u8> = None;
            items.iter().all(|item| {
                let item_kind = match item {
                    Value::Null => return false,
                    Value::Bool(_) => 1,
                    Value::Number(_) => 2,
                    Value::String(_) => 3,
                    Value::Array(_) => 4,
                    Value::Object(_) => 5,
                };
                match kind {
                    None => kind = Some(item_kind),
                    Some(seen) if seen == item_kind => {}
                    Some(_) => return false,
                }
                representable(item)
            })
        }
        Value::Object(entries) => entries.values().all(representable),
    }
}

/// Extract the normalized parse tree from toml bytes under
/// caller-provided safety budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-utf8 input or a document that
/// does not parse as TOML 1.0;
/// [`MediaError::ExtractTooLarge`] when the size pre-check trips.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = Instant::now();
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    let table = toml::from_str::<toml::Table>(text).map_err(|_| MediaError::ParseFailure)?;
    if started.elapsed() >= budgets.deadline {
        return Err(MediaError::ExtractTooLarge);
    }
    let tree = to_json(&toml::Value::Table(table));
    let head = serde_json::to_string(&tree).unwrap_or_default();
    let text_projection = truncate_chars(head, TEXT_PROJECTION_MAX_CHARS);
    Ok(Extracted { tree, text_projection, thumbnails: Vec::new() })
}

fn to_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(text) => Value::String(text.clone()),
        toml::Value::Integer(int) => Value::Number(Number::from(*int)),
        toml::Value::Float(float) => Number::from_f64(*float)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(flag) => Value::Bool(*flag),
        toml::Value::Datetime(datetime) => Value::String(datetime.to_string()),
        toml::Value::Array(items) => {
            Value::Array(items.iter().map(to_json).collect())
        }
        toml::Value::Table(table) => Value::Object(
            table
                .iter()
                .map(|(key, entry)| (key.clone(), to_json(entry)))
                .collect::<Map<String, Value>>(),
        ),
    }
}

/// Generate deterministic toml bytes from a validated spec.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec does not satisfy the toml
/// contract (see [`TomlSpec::from_generation`] - the serializer's own
/// refusal maps to the same typed error).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let parsed = TomlSpec::from_generation(spec)?;
    let text = toml::to_string(&parsed.value).map_err(|_| MediaError::InvalidSpec)?;
    Ok(text.into_bytes())
}

/// The toml diff profile: float epsilon only; no unordered pointers,
/// no ignored subtrees.
#[must_use]
pub fn profile() -> Tolerances {
    tolerances()
}

/// Verify produced toml bytes against the INDEPENDENT expected tree:
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

/// The toml deterministic projector (M10 seam): a thin wrap over the
/// extractor's bounded projection.
pub struct TomlProjector;

impl ProjectionProvider for TomlProjector {
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
