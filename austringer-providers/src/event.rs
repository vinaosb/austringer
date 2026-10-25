//! Typed provider stream events and the completion envelope.

use serde::{Deserialize, Serialize};

use crate::identity::ModelIdentity;

/// Token accounting for one completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens consumed by the prompt side.
    pub input_tokens: u64,
    /// Tokens produced by the completion side.
    pub output_tokens: u64,
    /// Prompt tokens served from the provider's read cache (billed at the
    /// cache-read rate, not the input rate). Zero when the provider does not
    /// report cache detail.
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// Prompt tokens written into the provider's cache this request
    /// (Anthropic cache writes bill above the input rate).
    #[serde(default)]
    pub cache_write_tokens: u64,
}

impl Default for Usage {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl Usage {
    /// Usage with input/output counts and no cache detail.
    #[must_use]
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        }
    }

    /// Usage with full cache detail.
    #[must_use]
    pub fn with_cache(
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
        }
    }
}

/// Why a model turn finished (the finish-reason vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    /// The model ended the turn of its own accord.
    EndTurn,
    /// The output token limit truncated the turn.
    Length,
    /// The turn ended requesting tool calls.
    ToolCall,
    /// The model refused to continue.
    Refusal,
}

/// One typed event on a provider stream. Every variant carries the
/// [`ModelIdentity`] the event came from, so substitution is observable
/// per-event, not just per-response.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderEvent {
    /// A fragment of assistant text.
    TextDelta {
        /// Identity the event originated from.
        model: ModelIdentity,
        /// The text fragment.
        text: String,
    },
    /// Opens a tool call: assigns the stable call id and tool name.
    ToolCallStart {
        /// Identity the event originated from.
        model: ModelIdentity,
        /// Stable correlation id for the whole call.
        call_id: String,
        /// Tool name.
        name: String,
    },
    /// A fragment of the tool call's JSON arguments.
    ToolCallDelta {
        /// Identity the event originated from.
        model: ModelIdentity,
        /// The stable call id this fragment belongs to.
        call_id: String,
        /// Concatenated in arrival order, the fragments form the arguments
        /// JSON document.
        arguments_fragment: String,
    },
    /// Token accounting for the response.
    Usage {
        /// Identity the event originated from.
        model: ModelIdentity,
        /// Reported token usage.
        usage: Usage,
    },
    /// The terminal event of a turn.
    Finish {
        /// Identity the event originated from.
        model: ModelIdentity,
        /// Why the turn finished.
        reason: FinishReason,
    },
}

impl ProviderEvent {
    /// The identity this event came from (identity contract: every event
    /// answers this).
    pub fn model(&self) -> &ModelIdentity {
        match self {
            Self::TextDelta { model, .. }
            | Self::ToolCallStart { model, .. }
            | Self::ToolCallDelta { model, .. }
            | Self::Usage { model, .. }
            | Self::Finish { model, .. } => model,
        }
    }
}

/// One fully-assembled tool call from a completed turn.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletedToolCall {
    /// Stable correlation id (matches the stream's `ToolCallStart`).
    pub call_id: String,
    /// Tool name.
    pub name: String,
    /// Parsed JSON arguments.
    pub arguments: serde_json::Value,
}

/// The completion/usage envelope for one finished turn: complete or an
/// error, never a silent partial (todo-5 seam contract (c)).
#[derive(Debug, Clone, PartialEq)]
pub struct CompletedTurn {
    /// Identity that served the whole turn (verified against every event).
    pub model: ModelIdentity,
    /// Full assistant text (empty when the turn only called tools).
    pub text: String,
    /// Tool calls requested by the turn, in request order.
    pub tool_calls: Vec<CompletedToolCall>,
    /// Token usage when the provider reported it.
    pub usage: Option<Usage>,
    /// Why the turn finished.
    pub finish_reason: FinishReason,
}
