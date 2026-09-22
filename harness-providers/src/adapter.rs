//! The binding to harness-core's `Inference` seam (todo-5 contract).
//!
//! The adapter keeps the seam's three invariants:
//!
//! (a) **llm-visible projection** — the model-context input is rendered from
//!     the sanctioned [`harness_core::event::llm_visible`] slice plus the
//!     stable focus block, in the same format the todo-5 test kit renders
//!     (so scripts and cassettes stay interchangeable);
//! (b) **cancel = abandon, not drain** — cancellation races the completion
//!     future via `select!`; losing drops the future, which drops the
//!     provider stream mid-flight;
//! (c) **complete-or-error** — a turn returns an envelope or a typed error
//!     rendered into [`InferenceError`]; silent partials are impossible
//!     because envelopes only exist post-`Finish`.
//!
//! The dependency direction is providers -> core, permanently: core owns the
//! seam, must never depend back on providers, and this binding lets the
//! seam's contract be TESTED against the resilient stack today.

use std::sync::Arc;

use async_trait::async_trait;
use harness_core::agent_loop::{
    AssistantTurn, Inference, InferenceError, InferenceRequest, PendingCall,
};
use harness_core::event::content::ContentBlock;
use harness_core::event::{Event, RecordedEvent};

use crate::identity::CompletionRequest;
use crate::provider::Provider;
use crate::retry::SeededJitter;
use crate::turn::{RecoveryConfig, ResumableTurn};

/// Drives any [`Provider`] through harness-core's `Inference` seam, with
/// retry, resume, and exactly-once recovery active underneath.
pub struct InferenceAdapter {
    provider: Arc<dyn Provider>,
    config: RecoveryConfig,
    jitter_seed: u64,
}

impl InferenceAdapter {
    /// Builds the adapter. Every inference call seeds a fresh
    /// [`SeededJitter`] from `jitter_seed`, so backoff sequences are
    /// reproducible per call in tests.
    pub fn new(provider: Arc<dyn Provider>, config: RecoveryConfig, jitter_seed: u64) -> Self {
        Self {
            provider,
            config,
            jitter_seed,
        }
    }
}

#[async_trait]
impl Inference for InferenceAdapter {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &harness_core::agent_loop::CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        let prompt = format!(
            "{}\n{}",
            request.focus_block,
            render_context(request.events)
        );
        let completion = CompletionRequest::new(self.provider.identity().clone(), prompt);
        let mut turn = ResumableTurn::start(
            self.provider.clone(),
            completion,
            self.config.clone(),
            Box::new(SeededJitter::new(self.jitter_seed)),
        );
        let completed = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                // Abandon: the in-flight future (and its provider stream) is
                // dropped at this branch; nothing is drained.
                return Err(InferenceError::Failure(
                    "cancelled: stream abandoned".to_owned(),
                ));
            }
            outcome = turn.complete() => outcome,
        }
        .map_err(|err| InferenceError::Failure(err.to_string()))?;
        let blocks = if completed.text.is_empty() {
            Vec::new()
        } else {
            vec![ContentBlock::Text {
                text: completed.text,
            }]
        };
        let tool_calls = completed
            .tool_calls
            .into_iter()
            .map(|call| PendingCall {
                call_id: call.call_id,
                tool: call.name,
                arguments: call.arguments,
            })
            .collect();
        Ok(AssistantTurn { blocks, tool_calls })
    }
}

/// Renders the llm-visible events exactly the way the todo-5 test kit's
/// `FakeApiInference` does — one line per event, deterministic, no internal
/// kinds (the `Condensation`/`Snapshot` arms are unreachable through the
/// filter and kept only for match exhaustiveness).
fn render_context(events: &[RecordedEvent]) -> String {
    harness_core::event::llm_visible(events)
        .map(|event| match event {
            Event::User { blocks } => format!("user: {}", block_text(blocks)),
            Event::Assistant { blocks } => format!("assistant: {}", block_text(blocks)),
            Event::ToolCall { call_id, tool, .. } => format!("tool-call {call_id} {tool}"),
            Event::ToolResult { call_id, .. } => format!("tool-result {call_id}"),
            Event::System { text } => format!("system: {text}"),
            Event::Condensation { .. } | Event::Snapshot { .. } => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn block_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image(_) | ContentBlock::Media(_) => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}
