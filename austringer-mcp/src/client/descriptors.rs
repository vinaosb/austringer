//! Remote-surface descriptors (rmcp-free value types) and the content
//! renderers shared by the client APIs.

use rmcp::model::{ContentBlock, Tool};
use serde_json::Value;

/// A remote tool's advertised contract (feeds the schema cache and the
/// registry mount; kept free of rmcp types).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolDescriptor {
    /// Tool name (unique within its server).
    pub name: String,
    /// One-line description (UNTRUSTED server text; rendered bounded).
    pub description: String,
    /// JSON-schema-shaped input description.
    pub input_schema: Value,
}

/// A remote resource's advertised contract.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDescriptor {
    /// Resource URI.
    pub uri: String,
    /// Human name (untrusted text).
    pub name: String,
}

/// A remote prompt's advertised contract.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptDescriptor {
    /// Prompt name.
    pub name: String,
    /// Description (untrusted text).
    pub description: String,
    /// Argument names.
    pub arguments: Vec<String>,
}

pub(super) fn describe_tool(tool: Tool) -> ToolDescriptor {
    ToolDescriptor {
        name: tool.name.clone().into_owned(),
        description: tool.description.clone().unwrap_or_default().into_owned(),
        input_schema: Value::Object(tool.input_schema.as_ref().clone()),
    }
}

pub(super) fn join_text(blocks: &[ContentBlock]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for block in blocks {
        if let ContentBlock::Text(text) = block {
            parts.push(text.text.clone());
        }
    }
    parts.join("\n")
}
