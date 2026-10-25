//! Tool schemas and the six invariant core-tool contracts.

use serde::{Deserialize, Serialize};
use serde_json::json;

/// The six core tools, serialized in exactly this canonical order in every
/// base-schema render. The set is INVARIANT: it never varies with autonomy
/// mode, active skills, or deferred-loaded tools.
pub const CORE_TOOLS: [&str; 6] = [
    "view_window",
    "edit_lines",
    "execute_command",
    "lsp_query",
    "tool_search",
    "task_completed",
];

/// Name of the deferred-loading search tool (one of [`CORE_TOOLS`]).
pub const TOOL_SEARCH: &str = "tool_search";

/// Name of the loop-enforced completion tool (one of [`CORE_TOOLS`]). The
/// agent loop intercepts it at the ToolExecution operation; the registry
/// never executes it.
pub const TASK_COMPLETED: &str = "task_completed";

/// A tool's contract: name, one-line purpose, and a JSON-schema-shaped
/// parameter description.
///
/// Serialization is byte-stable by construction: struct fields serialize in
/// declaration order and dynamic `parameters` objects serialize with sorted
/// keys (serde_json's default BTreeMap ordering). No timestamp or OS path
/// ever enters the serialized form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    /// The tool name (the dispatch key).
    pub name: String,
    /// What the tool does, for both the model and the aux table.
    pub description: String,
    /// JSON-schema-shaped parameter description.
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    /// The first line of the description (the aux-table entry; never
    /// multi-line).
    #[must_use]
    pub fn one_line_description(&self) -> String {
        self.description
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned()
    }
}

/// The day-one contract for a core tool by name (all version 1). The real
/// executors arrive in later todos (16, 18, 19); the schemas are fixed now
/// so the serialized base schema is stable from day one.
#[must_use]
pub fn core_schema(name: &str) -> ToolSchema {
    let (description, parameters) = match name {
        "view_window" => (
            "Read a bounded window of a file (up to 100 lines, 1-based) by path, with an optional start line.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read." },
                    "start_line": { "type": "integer", "minimum": 1, "description": "1-based first line to show." }
                },
                "required": ["path"]
            }),
        ),
        "edit_lines" => (
            "Apply a set of line edits to a file with uniqueness-validated anchors; the result reports changed lines with a context fringe.",
            json!({
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
                                "new_text": { "type": "string" }
                            },
                            "required": ["line", "old_text", "new_text"]
                        }
                    }
                },
                "required": ["path", "edits"]
            }),
        ),
        "execute_command" => (
            "Run a shell command under the PTY supervisor with a timeout ceiling; output is bounded before entering context.",
            json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Command line to run." },
                    "timeout_ms": { "type": "integer", "minimum": 1, "description": "Optional timeout ceiling." }
                },
                "required": ["command"]
            }),
        ),
        "lsp_query" => (
            "Query the language server for a symbol at a location: definition, references, or hover, returning compact locations.",
            json!({
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["definition", "references", "hover"] },
                    "file": { "type": "string" },
                    "line": { "type": "integer", "minimum": 1 },
                    "character": { "type": "integer", "minimum": 0 }
                },
                "required": ["kind", "file", "line", "character"]
            }),
        ),
        "tool_search" => (
            "Search the deferred tool catalog by name or description; matching tool schemas are hydrated into this call's own result, never the system prompt.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "minLength": 1, "description": "Search text." }
                },
                "required": ["query"]
            }),
        ),
        "task_completed" => (
            "Signal task completion; enforced by the loop only after every focus-chain todo is closed and the plan gate is clear.",
            json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "What was accomplished." }
                },
                "required": ["summary"]
            }),
        ),
        _ => ("", json!({})),
    };
    ToolSchema {
        name: name.to_owned(),
        description: description.to_owned(),
        parameters,
    }
}
