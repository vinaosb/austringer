//! Inference fakes for agent-loop (todo 5) tests: a scripted queue and a
//! `FakeApi`-backed streaming adapter that abandons its stream on cancel.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harness_core::agent_loop::{
    AssistantTurn, Inference, InferenceError, InferenceRequest, PendingCall,
};
use harness_core::event::content::ContentBlock;
use harness_core::event::{Event, RecordedEvent};
use harness_test_support::provider::{FakeApi, ProviderRequest};
use harness_test_support::sse::SseParser;

/// One captured inference request (owned copy for assertions).
#[derive(Clone, Debug, PartialEq)]
pub struct Captured {
    /// The stable focus-chain environment block as sent.
    pub focus_block: String,
    /// Deterministic rendering of the llm-visible context events.
    pub context: String,
}

/// Renders the llm-visible events the way the fake adapters see them.
pub fn render_context(events: &[RecordedEvent]) -> String {
    harness_core::event::llm_visible(events)
        .map(|event| match event {
            Event::User { blocks } => {
                format!("user: {}", block_text(blocks))
            }
            Event::Assistant { blocks } => {
                format!("assistant: {}", block_text(blocks))
            }
            Event::ToolCall { call_id, tool, .. } => format!("tool-call {call_id} {tool}"),
            Event::ToolResult { call_id, .. } => format!("tool-result {call_id}"),
            Event::System { text } => format!("system: {text}"),
            Event::Condensation { .. } | Event::Snapshot { .. } => "internal".to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn block_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image(_) => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// An assistant turn of plain text.
pub fn text_turn(text: &str) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: Vec::new(),
    }
}

/// An assistant turn requesting one tool call.
pub fn call_turn(call_id: &str, tool: &str, arguments: serde_json::Value) -> AssistantTurn {
    AssistantTurn {
        blocks: Vec::new(),
        tool_calls: vec![PendingCall {
            call_id: call_id.to_owned(),
            tool: tool.to_owned(),
            arguments,
        }],
    }
}

/// Queued-result inference fake: serves scripted turns/errors in order.
#[derive(Debug, Default)]
pub struct ScriptedInference {
    queue: Mutex<VecDeque<Result<AssistantTurn, InferenceError>>>,
    seen: Mutex<Vec<Captured>>,
}

impl ScriptedInference {
    /// Serves only successful scripted turns.
    pub fn new(turns: Vec<AssistantTurn>) -> Self {
        Self::from_results(turns.into_iter().map(Ok).collect())
    }

    /// Serves scripted results (errors allowed) in order.
    pub fn from_results(results: Vec<Result<AssistantTurn, InferenceError>>) -> Self {
        Self {
            queue: Mutex::new(results.into_iter().collect()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Every captured request so far.
    pub fn seen(&self) -> Vec<Captured> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn capture(&self, request: &InferenceRequest<'_>) {
        let captured = Captured {
            focus_block: request.focus_block.to_owned(),
            context: render_context(request.events),
        };
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(captured);
        }
    }

    fn next(&self) -> Result<AssistantTurn, InferenceError> {
        self.queue
            .lock()
            .map_err(|_| InferenceError::Failure("script poisoned".to_owned()))
            .and_then(|mut q| {
                q.pop_front()
                    .unwrap_or_else(|| Err(InferenceError::Failure("script exhausted".to_owned())))
            })
    }
}

#[async_trait]
impl Inference for ScriptedInference {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &harness_core::agent_loop::CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        self.capture(request);
        self.next()
    }
}

/// `FakeApi`-backed streaming adapter: builds a prompt from the focus block +
/// rendered context, streams the scripted SSE body, and abandons the stream
/// the moment the cancel token fires (never waits out a delay).
#[derive(Clone)]
pub struct FakeApiInference {
    api: Arc<FakeApi>,
    model: String,
    seen: Arc<Mutex<Vec<Captured>>>,
}

impl FakeApiInference {
    /// Wraps a fake api + model name.
    pub fn new(api: Arc<FakeApi>, model: &str) -> Self {
        Self {
            api,
            model: model.to_owned(),
            seen: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Every captured request so far.
    pub fn seen(&self) -> Vec<Captured> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Inference for FakeApiInference {
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
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(Captured {
                focus_block: request.focus_block.to_owned(),
                context: render_context(request.events),
            });
        }
        let mut stream = self
            .api
            .stream(&ProviderRequest::new(&self.model, &prompt))
            .map_err(|error| InferenceError::Failure(error.to_string()))?;
        let mut parser = SseParser::new();
        let mut abandoned = false;
        loop {
            if cancel.is_cancelling() {
                abandoned = true;
                break;
            }
            let chunk = tokio::select! {
                biased;
                chunk = stream.next_chunk() => Some(chunk),
                _ = cancel.cancelled() => None,
            };
            match chunk {
                Some(Some(bytes)) => parser.feed(&bytes),
                // Stream ended normally.
                Some(None) => break,
                // Cancelled: abandon the stream, never drain it.
                None => {
                    abandoned = true;
                    break;
                }
            }
        }
        if abandoned {
            return Err(InferenceError::Failure(
                "stream abandoned: cancelling".to_owned(),
            ));
        }
        let mut text = String::new();
        let mut calls = Vec::new();
        for event in parser.events() {
            match event.event.as_str() {
                "text_delta" => {
                    if let Some(delta) = event.data.get("text").and_then(|v| v.as_str()) {
                        text.push_str(delta);
                    }
                }
                "tool_call" => {
                    let call_id = event.data.get("call_id").and_then(|v| v.as_str());
                    let name = event.data.get("name").and_then(|v| v.as_str());
                    if let (Some(call_id), Some(name)) = (call_id, name) {
                        calls.push(PendingCall {
                            call_id: call_id.to_owned(),
                            tool: name.to_owned(),
                            arguments: event
                                .data
                                .get("arguments")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(AssistantTurn {
            blocks: vec![ContentBlock::Text { text }],
            tool_calls: calls,
        })
    }
}
