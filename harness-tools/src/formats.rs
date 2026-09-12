//! Edit-format routing per model family (plan todo 16, clause 11;
//! research §33): ship the edit-tool format each model family was
//! RL-trained on — patch-style (unified-diff flavored) for the
//! OpenAI family, string-replace/anchor flavored for the Anthropic
//! family — plus one schema variant per format.
//!
//! Schema-similarity rule (documented, drives the routing table): a
//! custom edit schema whose shape is closer to the model's trained edit
//! format produces fewer malformed calls — schema priors leak across
//! generations, so we route to the trained shape instead of teaching a
//! new one. The T34 profile subsystem consumes this table; nothing here
//! depends on harness-core (the anchor variant mirrors the day-one
//! `edit_lines` contract fixed in todo 6).

use serde::{Deserialize, Serialize};
use serde_json::json;

/// Model family with a distinct trained edit format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelFamily {
    /// OpenAI-family models (patch-style edits in training).
    OpenAi,
    /// Anthropic-family models (string-replace/anchor-style edits).
    Anthropic,
}

/// The edit format a schema variant speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditFormat {
    /// Unified-diff flavored payload (`path` + `patch`).
    UnifiedPatch,
    /// Anchor + old/new text pairs (the core `edit_lines` shape).
    AnchorReplace,
}

/// The routing table: family → default edit format. Exposed as a typed
/// const so the profile subsystem can embed it without re-derivation.
pub const EDIT_FORMAT_ROUTING: &[(ModelFamily, EditFormat)] = &[
    (ModelFamily::OpenAi, EditFormat::UnifiedPatch),
    (ModelFamily::Anthropic, EditFormat::AnchorReplace),
];

/// Default [`EditFormat`] for a family (from [`EDIT_FORMAT_ROUTING`]).
#[must_use]
pub fn default_format(family: ModelFamily) -> EditFormat {
    EDIT_FORMAT_ROUTING
        .iter()
        .find(|(mapped, _)| *mapped == family)
        .map_or(EditFormat::AnchorReplace, |(_, format)| *format)
}

/// One schema variant: the JSON-schema-shaped parameter description an
/// edit tool exposes when speaking this format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditFormatSchema {
    /// The format this schema speaks.
    pub format: EditFormat,
    /// Tool name the family's edit tool takes.
    pub name: String,
    /// One-line summary (aux-table vocabulary).
    pub summary: String,
    /// JSON-schema-shaped parameters.
    pub parameters: serde_json::Value,
}

/// The schema variant for an [`EditFormat`].
#[must_use]
pub fn edit_schema_variant(format: EditFormat) -> EditFormatSchema {
    match format {
        EditFormat::UnifiedPatch => EditFormatSchema {
            format,
            name: "apply_patch".to_owned(),
            summary: "Apply a unified-diff patch to a file; hunks anchor on context lines.".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to patch." },
                    "patch": {
                        "type": "string",
                        "description": "Unified diff (hunks with @@ headers and context lines)."
                    }
                },
                "required": ["path", "patch"]
            }),
        },
        EditFormat::AnchorReplace => EditFormatSchema {
            format,
            name: "edit_lines".to_owned(),
            summary: "Apply anchored line edits with uniqueness validation; result reports changed lines with a context fringe.".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to edit." },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "line": { "type": "integer", "minimum": 1 },
                                "old_text": { "type": "string" },
                                "new_text": { "type": "string" },
                                "hash": {
                                    "type": "string",
                                    "description": "Optional hashline anchor for drift detection."
                                }
                            },
                            "required": ["line", "old_text", "new_text"]
                        }
                    }
                },
                "required": ["path", "edits"]
            }),
        },
    }
}
