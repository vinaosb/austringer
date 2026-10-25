//! Base-schema rendering: the invariant six-tool serialization plus the
//! markdown aux table (impl split of [`ToolRegistry`] by responsibility).

use serde::Serialize;

use super::registry::ToolRegistry;
use super::schema::{CORE_TOOLS, ToolSchema};
use super::version::PinnedToolSet;

/// The serialized base-schema document: the six core schemas plus the
/// markdown aux table. Field order is the stability contract.
#[derive(Serialize)]
struct BaseSchemaDoc<'a> {
    core_tools: &'a [ToolSchema],
    aux_tools_table: String,
}

impl ToolRegistry {
    /// Renders the six core schemas (canonical order, pinned versions) as
    /// a compact JSON array. Invariant: this byte string never varies with
    /// autonomy mode, active skills, or deferred-loaded tools.
    #[must_use]
    pub fn render_core_schemas(&self, pinned: &PinnedToolSet) -> String {
        let schemas: Vec<ToolSchema> = CORE_TOOLS
            .iter()
            .filter_map(|name| self.pinned_schema(pinned, name).cloned())
            .collect();
        // Infallible by type (strings + Values).
        serde_json::to_string(&schemas).unwrap_or_default()
    }

    /// Renders the serialized base schema: the six core schemas plus the
    /// markdown aux table of every other pinned tool. Byte-stable for a
    /// pinned session regardless of what the session hydrates or calls.
    #[must_use]
    pub fn render_base_schema(&self, pinned: &PinnedToolSet) -> String {
        let schemas: Vec<ToolSchema> = CORE_TOOLS
            .iter()
            .filter_map(|name| self.pinned_schema(pinned, name).cloned())
            .collect();
        let doc = BaseSchemaDoc {
            core_tools: &schemas,
            aux_tools_table: self.aux_table(pinned),
        };
        // Infallible by type (strings + Values).
        serde_json::to_string(&doc).unwrap_or_default()
    }

    /// The markdown aux table: name + one-line description of every pinned
    /// non-core tool, name-sorted. Header-only when none exist.
    #[must_use]
    pub fn aux_table(&self, pinned: &PinnedToolSet) -> String {
        let mut table = String::from("| tool | description |\n| --- | --- |");
        for name in pinned.tool_names() {
            if CORE_TOOLS.contains(&name) {
                continue;
            }
            if let Some(schema) = self.pinned_schema(pinned, name) {
                table.push_str(&format!(
                    "\n| {} | {} |",
                    schema.name,
                    schema.one_line_description()
                ));
            }
        }
        table
    }
}
