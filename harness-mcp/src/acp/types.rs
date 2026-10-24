//! ACP v1 wire types (subset actually exercised by the agent-side
//! surface; field names are EXACTLY the spec's camelCase — see the
//! module docs in `agent.rs` for the name mapping disclosure).

use serde::{Deserialize, Serialize};

/// ACP v1 protocol version (integer on the wire).
pub const PROTOCOL_VERSION: u64 = 1;

/// One prompt content block (text or an @-mention link).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AcpContentBlock {
    /// Plain text.
    Text {
        /// The text.
        text: String,
    },
    /// A reference (e.g. an @-mention); resolved via `fs/read_text_file`.
    ResourceLink {
        /// Display name.
        name: String,
        /// Referenced URI (typically `file://…`).
        uri: String,
    },
}

/// One prompt: its content blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct AcpPrompt {
    /// The blocks the IDE sent.
    pub blocks: Vec<AcpContentBlock>,
}

/// Why a prompt turn ended (spec values; lives on the `session/prompt`
/// response, NOT on notifications).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The agent finished its turn.
    EndTurn,
    /// The context window filled.
    MaxTokens,
    /// The turn-request ceiling hit.
    MaxTurnRequests,
    /// The agent refused.
    Refusal,
    /// The IDE cancelled the session.
    Cancelled,
}

/// One `session/update` payload (tagged by `sessionUpdate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
pub enum SessionUpdate {
    /// A chunk of agent message content.
    AgentMessageChunk {
        /// The chunk.
        content: AcpContentBlock,
    },
    /// A chunk echoed from the user message.
    UserMessageChunk {
        /// The chunk.
        content: AcpContentBlock,
    },
    /// A tool call (edit requests ride this variant with diff content).
    ToolCall {
        /// Correlation id.
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        /// Human title.
        title: String,
        /// Coarse kind (read/edit/execute/…).
        kind: String,
        /// Structured content (diffs).
        content: Vec<ToolCallContent>,
    },
}

impl SessionUpdate {
    /// Convenience: an agent text chunk.
    #[must_use]
    pub fn agent_message_chunk(text: &str) -> Self {
        Self::AgentMessageChunk {
            content: AcpContentBlock::Text {
                text: text.to_owned(),
            },
        }
    }

    /// Convenience: an edit-request tool call carrying one file diff.
    #[must_use]
    pub fn edit_request(path: &str, old_text: Option<&str>, new_text: &str) -> Self {
        Self::ToolCall {
            tool_call_id: format!("edit-{}", blake3::hash(path.as_bytes()).to_hex()),
            title: format!("edit {path}"),
            kind: "edit".to_owned(),
            content: vec![ToolCallContent::Diff {
                path: path.to_owned(),
                old_text: old_text.map(str::to_owned),
                new_text: new_text.to_owned(),
            }],
        }
    }
}

/// Structured tool-call content (the file-edit representation — ACP has
/// no separate document-change event stream).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    /// A file diff.
    Diff {
        /// Target path.
        path: String,
        /// Previous text (None = new file).
        #[serde(rename = "oldText")]
        old_text: Option<String>,
        /// Next text.
        #[serde(rename = "newText")]
        new_text: String,
    },
}

/// Coarse permission-option kinds (the spec's closed vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOptionKind {
    /// Approve this one call.
    AllowOnce,
    /// Approve and remember for the session.
    AllowAlways,
    /// Refuse this one call.
    RejectOnce,
}

/// One option offered on a `session/request_permission` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionOption {
    /// Machine id (echoed back on selection).
    #[serde(rename = "optionId")]
    pub option_id: String,
    /// Human label.
    pub name: String,
    /// Coarse kind.
    pub kind: PermissionOptionKind,
}

impl PermissionOption {
    /// Convenience constructor from a kind (id = kind name, name =
    /// humanized label).
    #[must_use]
    pub fn from_kind(kind: PermissionOptionKind) -> Self {
        let (option_id, name) = match kind {
            PermissionOptionKind::AllowOnce => ("allow_once", "Allow Once"),
            PermissionOptionKind::AllowAlways => ("allow_always", "Allow Always"),
            PermissionOptionKind::RejectOnce => ("reject_once", "Reject Once"),
        };
        Self {
            option_id: option_id.to_owned(),
            name: name.to_owned(),
            kind,
        }
    }
}

/// The IDE's answer to a `session/request_permission` request (the
/// response's `outcome` union).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PermissionOutcome {
    /// The IDE picked an offered option.
    Selected {
        /// The chosen option's id.
        #[serde(rename = "optionId")]
        option_id: String,
    },
    /// The IDE dismissed the request without choosing.
    Cancelled,
}
