//! Pack aggregation: `_shared` partials interpolated into skill bodies at
//! invoke time, and the findings-schema validator for pack outputs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::SkillError;

/// A `{{shared:name}}` partial that was resolved into a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPartial {
    /// Partial name (file stem under `_shared/`).
    pub name: String,
}

/// Replaces every `{{shared:name}}` marker in `body` with the partial's
/// content. Missing partials are typed errors — a body must never render
/// with unresolved or silently-dropped markers.
pub fn resolve_shared(
    body: &str,
    partials: &BTreeMap<String, String>,
    shared_dir: &std::path::Path,
) -> Result<(String, Vec<ResolvedPartial>), SkillError> {
    let mut out = String::with_capacity(body.len());
    let mut resolved = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("{{shared:") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "{{shared:".len()..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = &after[..end];
        let Some(content) = partials.get(name) else {
            return Err(SkillError::SharedPartialNotFound {
                name: name.to_string(),
                path: shared_dir.to_path_buf(),
            });
        };
        out.push_str(content);
        resolved.push(ResolvedPartial {
            name: name.to_string(),
        });
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok((out, resolved))
}

/// Findings-schema field types (a typed, JSON-schema-ish subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// JSON string.
    Str,
    /// JSON number.
    Num,
    /// JSON boolean.
    Bool,
    /// JSON array.
    Arr,
}

impl FieldType {
    /// Maps a schema `"type"` string; `None` when unknown.
    pub fn from_schema_str(raw: &str) -> Option<Self> {
        match raw {
            "string" => Some(Self::Str),
            "number" => Some(Self::Num),
            "bool" | "boolean" => Some(Self::Bool),
            "array" => Some(Self::Arr),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Str => "string",
            Self::Num => "number",
            Self::Bool => "boolean",
            Self::Arr => "array",
        }
    }

    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Str => value.is_string(),
            Self::Num => value.is_number(),
            Self::Bool => value.is_boolean(),
            Self::Arr => value.is_array(),
        }
    }
}

/// One field rule in a findings schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSpec {
    /// Field type.
    #[serde(rename = "type")]
    pub ftype: String,
    /// Whether the field must be present.
    #[serde(default)]
    pub required: bool,
    /// Allowed values, when the field is an enum.
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enumeration: Option<Vec<String>>,
}

/// A pack's findings schema: required/optional fields with types and
/// optional enum constraints. Unknown fields in outputs are allowed
/// (forward compatibility).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingsSchema {
    /// Field name → rule, sorted stable.
    pub fields: BTreeMap<String, FieldSpec>,
}

impl FindingsSchema {
    /// Parses a findings-schema JSON document of the shape
    /// `{"fields": {name: {"type": ..., "required": ..., "enum": [...]}}}`.
    pub fn parse(json: &str) -> Result<Self, SkillError> {
        #[derive(Deserialize)]
        struct RawSchema {
            fields: BTreeMap<String, FieldSpec>,
        }
        let raw: RawSchema = serde_json::from_str(json).map_err(|e| SkillError::SchemaParse {
            detail: e.to_string(),
        })?;
        for (name, spec) in &raw.fields {
            if FieldType::from_schema_str(&spec.ftype).is_none() {
                return Err(SkillError::SchemaParse {
                    detail: format!("field {name:?} has unknown type {:?}", spec.ftype),
                });
            }
        }
        Ok(Self { fields: raw.fields })
    }
}

/// A findings-schema violation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaViolation {
    /// A required field is absent.
    #[error("missing required field {field:?}")]
    MissingField {
        /// The absent field.
        field: String,
    },
    /// A field's JSON type does not match the schema.
    #[error("field {field:?} expected {expected}, found {found}")]
    TypeMismatch {
        /// The offending field.
        field: String,
        /// Schema type label.
        expected: &'static str,
        /// Actual JSON type.
        found: &'static str,
    },
    /// A field's value is outside the allowed enum.
    #[error("field {field:?} value {value:?} not in allowed set")]
    EnumMismatch {
        /// The offending field.
        field: String,
        /// The rejected value.
        value: String,
    },
}

/// Validates a findings document against a schema; all violations are
/// returned together (never first-only) so pack output reports are complete.
pub fn validate_findings(schema: &FindingsSchema, doc: &Value) -> Result<(), Vec<SchemaViolation>> {
    let empty = BTreeMap::new();
    let obj = doc.as_object().map(|o| {
        o.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<BTreeMap<String, Value>>()
    });
    let obj = obj.as_ref().unwrap_or(&empty);
    let mut violations = Vec::new();
    for (name, spec) in &schema.fields {
        let Some(ftype) = FieldType::from_schema_str(&spec.ftype) else {
            continue;
        };
        match obj.get(name) {
            None => {
                if spec.required {
                    violations.push(SchemaViolation::MissingField {
                        field: name.clone(),
                    });
                }
            }
            Some(value) => {
                if !ftype.accepts(value) {
                    violations.push(SchemaViolation::TypeMismatch {
                        field: name.clone(),
                        expected: ftype.label(),
                        found: json_type(value),
                    });
                } else if let (FieldType::Str, Some(allowed)) = (ftype, &spec.enumeration) {
                    let v = value.as_str().unwrap_or_default().to_string();
                    if !allowed.contains(&v) {
                        violations.push(SchemaViolation::EnumMismatch {
                            field: name.clone(),
                            value: v,
                        });
                    }
                }
            }
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
