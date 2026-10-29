//! The same-model condenser bridge: the session's composed inference
//! stack shared between the loop and the doctrine condense seam, so a
//! condense call rides the very stack (and spend accounting) the
//! session's turns ride. Stages and inference never overlap, so the
//! shared lock is uncontended by construction.

use std::sync::Arc;

use async_trait::async_trait;

use austringer_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest,
};
use austringer_core::compaction::condense::{DoctrineCondenseRequest, DoctrineCondenser, ModelId};
use austringer_core::event::RecordedEvent;
use austringer_core::eviction::CondenseError;

/// The session's inference stack behind one shared handle.
pub struct SharedStack {
    stack: Arc<tokio::sync::Mutex<Box<dyn Inference>>>,
}

impl SharedStack {
    /// Wraps the composed stack.
    pub fn new(stack: Box<dyn Inference>) -> Self {
        Self {
            stack: Arc::new(tokio::sync::Mutex::new(stack)),
        }
    }

    /// The engine-side handle over the shared stack.
    pub fn handle(&self) -> Box<dyn Inference> {
        Box::new(SharedInference {
            stack: Arc::clone(&self.stack),
        })
    }

    /// The same-model condenser registration over the shared stack.
    pub fn condenser(&self, model: ModelId) -> (ModelId, Arc<dyn DoctrineCondenser>) {
        (
            model,
            Arc::new(InferenceCondenser {
                stack: Arc::clone(&self.stack),
            }),
        )
    }
}

struct SharedInference {
    stack: Arc<tokio::sync::Mutex<Box<dyn Inference>>>,
}

#[async_trait]
impl Inference for SharedInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        self.stack.lock().await.infer(request, cancel).await
    }
}

struct InferenceCondenser {
    stack: Arc<tokio::sync::Mutex<Box<dyn Inference>>>,
}

#[async_trait]
impl DoctrineCondenser for InferenceCondenser {
    async fn condense(
        &self,
        request: DoctrineCondenseRequest<'_>,
    ) -> Result<String, CondenseError> {
        let span: Vec<RecordedEvent> = request.span.to_vec();
        // A condense call is a text task: no system surface, no declared
        // tools — the session stack must answer with prose, not calls.
        let condense_request = InferenceRequest {
            events: &span,
            focus_block: request.prompt,
            system: "",
            tools: &[],
        };
        let cancel = CancelToken::new();
        let turn = {
            let mut stack = self.stack.lock().await;
            stack.infer(&condense_request, &cancel).await
        }
        .map_err(|error| CondenseError(error.message().to_owned()))?;
        let text = turn
            .blocks
            .iter()
            .map(|block| block.text_marker())
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            return Err(CondenseError(
                "condense produced no text: the session's model returned an empty turn".into(),
            ));
        }
        Ok(text)
    }
}
