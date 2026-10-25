//! Scripted reply variants queued behind a rule.

use crate::sse::SseScript;

/// One queued reply for a fake-provider rule.
#[derive(Clone, Debug)]
pub enum ScriptedReply {
    /// Successful SSE stream.
    Stream(SseScript),
    /// 200 OK with zero choices.
    NoChoices,
    /// Truncated by the output token limit.
    OutputLimit,
    /// Rejected for exceeding the context window.
    ContextLimit,
    /// Generic HTTP 500.
    ServerError,
}
