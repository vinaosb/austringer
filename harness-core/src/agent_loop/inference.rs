//! The inference seam: model-context events in, assistant turn out.
//!
//! This is deliberately the thinnest possible trait so todo 5's loop can be
//! driven by `harness-test-support`'s `FakeApi` behind a small adapter, with
//! the REAL `Provider` trait (todo 9) sitting behind an adapter of its own.
//!
//! # Risk note (todo 9 must align)
//!
//! Todo 9 defines the dyn-safe streaming `Provider`. Its adapter must keep
//! this seam's contracts: (a) the request carries the sanctioned
//! llm-visible event slice plus the stable focus block; (b) implementations
//! poll the cancel token while streaming and abandon the stream on
//! cancellation (never drain it); (c) the returned turn is complete or an
//! error — never a silent partial. Mid-stream recovery and idle timeouts
//! live in todo 9, not here.

use async_trait::async_trait;

use super::cancel::CancelToken;
use crate::event::RecordedEvent;
use crate::event::content::ContentBlock;

/// One tool invocation requested by the assistant.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingCall {
    /// Correlation id linking the call to its future result.
    pub call_id: String,
    /// Tool name.
    pub tool: String,
    /// Tool arguments as arbitrary JSON.
    pub arguments: serde_json::Value,
}

/// A complete assistant turn: content plus requested tool calls.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantTurn {
    /// Assistant content blocks (text today; images when models emit them).
    pub blocks: Vec<ContentBlock>,
    /// Tool calls requested by this turn (empty = turn ended).
    pub tool_calls: Vec<PendingCall>,
}

/// What one inference call sees: the model-context events (the caller's
/// sanctioned llm-visible slice of the log) plus the stable focus-chain
/// environment block, re-injected every turn.
#[derive(Debug)]
pub struct InferenceRequest<'a> {
    /// Model-context events. Consumers MUST respect the event taxonomy's
    /// [`crate::event::llm_visible`] filter when rendering.
    pub events: &'a [RecordedEvent],
    /// The byte-stable focus-chain block for the current todo state.
    pub focus_block: &'a str,
}

/// Why an inference call failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InferenceError {
    /// The provider call failed (transport, HTTP, protocol, or an
    /// abandoned stream). Todo 9 replaces the payload with typed variants.
    #[error("inference failed: {0}")]
    Failure(String),
}

/// The inference seam. See the module docs for the todo-9 contract.
#[async_trait]
pub trait Inference: Send {
    /// Produces the assistant turn for the given request. Implementations
    /// poll `cancel` while streaming and abandon the stream on cancellation
    /// rather than draining it.
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError>;
}

/// Tool name of the reserved task-completion operation. The loop intercepts
/// it at the ToolExecution operation instead of executing it (todo 6 wires
/// the real tool).
pub const TASK_COMPLETED_TOOL: &str = "task_completed";
