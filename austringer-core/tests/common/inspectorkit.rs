//! Shared fixtures for inspector (todo 7) tests: a recording executor, a
//! capturing/scripted ask policy, and event-text extractors.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use austringer_core::agent_loop::stages::{StageCtx, StageError, ToolExecutor, ToolOutcome};
use austringer_core::agent_loop::{AssistantTurn, PendingCall};
use austringer_core::event::{ContentBlock, Event, RecordedEvent};
use austringer_core::inspectors::chain::InspectorConfig;
use austringer_core::inspectors::permission::{Effect, Rule};
use austringer_core::inspectors::store::{AskPolicy, AskRequest, AskResolution};

/// One synthetic tool call.
pub fn call(tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: format!("call-{tool}"),
        tool: tool.to_owned(),
        arguments,
    }
}

/// An assistant turn requesting several tool calls at once.
pub fn calls_turn(calls: Vec<(&str, &str, serde_json::Value)>) -> AssistantTurn {
    AssistantTurn {
        blocks: Vec::new(),
        tool_calls: calls
            .into_iter()
            .map(|(id, tool, arguments)| PendingCall {
                call_id: id.to_owned(),
                tool: tool.to_owned(),
                arguments,
            })
            .collect(),
        usage: None,
    }
}

/// Every tool-result event rendered as `call-id: text`.
pub fn tool_result_texts(events: &[RecordedEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|recorded| match &recorded.event {
            Event::ToolResult { call_id, blocks } => Some(format!(
                "{call_id}: {}",
                blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        ContentBlock::Image(_) | ContentBlock::Media(_) => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
            _ => None,
        })
        .collect()
}

/// Every system-event text, in order.
pub fn system_texts(events: &[RecordedEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|recorded| match &recorded.event {
            Event::System { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Default config plus one allow-everything rule for `tool_pattern`.
pub fn allow_config(tool_pattern: &str) -> InspectorConfig {
    let mut config = InspectorConfig::default();
    config
        .permission
        .rules
        .push(Rule::tool(Effect::Allow, tool_pattern).expect("rule"));
    config
}

/// Records every call it executes; returns "executed-ok".
#[derive(Clone, Default)]
pub struct RecordingExecutor {
    calls: Arc<Mutex<Vec<PendingCall>>>,
}

impl RecordingExecutor {
    /// The calls that reached execution.
    pub fn calls(&self) -> Vec<PendingCall> {
        self.calls.lock().expect("recorder").clone()
    }
}

#[async_trait]
impl ToolExecutor for RecordingExecutor {
    async fn execute(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        self.calls.lock().expect("recorder").push(call);
        Ok(ToolOutcome::text("executed-ok"))
    }
}

/// Scripted ask policy: records every ask, answers from the queue (an
/// empty queue refuses).
#[derive(Clone, Default)]
pub struct CaptureAsks {
    asked: Arc<Mutex<Vec<AskRequest>>>,
    queue: Arc<Mutex<VecDeque<AskResolution>>>,
}

impl CaptureAsks {
    /// A policy that answers with `resolutions` in order.
    pub fn with(resolutions: Vec<AskResolution>) -> Self {
        Self {
            asked: Arc::default(),
            queue: Arc::new(Mutex::new(resolutions.into_iter().collect())),
        }
    }

    /// Every ask routed so far.
    pub fn asked(&self) -> Vec<AskRequest> {
        self.asked.lock().expect("asked").clone()
    }
}

#[async_trait]
impl AskPolicy for CaptureAsks {
    async fn resolve(&mut self, request: AskRequest) -> AskResolution {
        self.asked.lock().expect("asked").push(request);
        self.queue
            .lock()
            .expect("queue")
            .pop_front()
            .unwrap_or(AskResolution::Refuse)
    }
}
