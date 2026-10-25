//! Shared kit for recovery tests: configs, scripts, and the turn driver
//! that collects delivered events (sequences, never booleans).

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use super::{FakeBridge, Plan};
use austringer_providers::error::ProviderStreamError;
use austringer_providers::event::{CompletedTurn, ProviderEvent};
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::retry::{FixedJitter, JitterSource};
use austringer_providers::turn::{RecoveryConfig, ResumableTurn};
use austringer_test_support::provider::FakeApi;
use austringer_test_support::sse::{
    SseEvent, SseScript, SseScriptBuilder, ev_done, ev_message_start, ev_text_delta,
    ev_tool_call_delta, ev_usage,
};

pub fn identity() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-4-5")
}

pub fn fast_config(max_resumes: u32) -> RecoveryConfig {
    RecoveryConfig {
        retry: austringer_providers::retry::RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(4),
            multiplier: 2.0,
        },
        max_resumes,
        idle_timeout: Duration::from_secs(60),
    }
}

pub fn jitter() -> Box<dyn JitterSource> {
    Box::new(FixedJitter::new([0.5]))
}

/// The reference script: lead text + one fragmented tool call + usage +
/// tool-call finish.
pub fn reference_script() -> SseScript {
    tool_script("grep -r TODO src/", "Running grep")
}

pub fn tool_script(args: &str, lead: &str) -> SseScript {
    let halves = ["{\"cmd\"", &format!(":{args:?}}}")];
    let mut builder = SseScriptBuilder::new().event(ev_message_start());
    if !lead.is_empty() {
        builder = builder.event(ev_text_delta(lead));
    }
    builder = builder.event(SseEvent::new(
        "tool_call_start",
        serde_json::json!({ "call_id": "c1", "name": "bash" }),
    ));
    for half in halves {
        builder = builder.event(ev_tool_call_delta("c1", half));
    }
    builder
        .event(ev_usage(11, 7))
        .event(SseEvent::new(
            "finish",
            serde_json::json!({ "reason": "tool_call" }),
        ))
        .event(ev_done())
        .build()
}

/// Cumulative byte offsets of each rendered frame in the script.
pub fn frame_offsets(script: &SseScript) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut acc = 0;
    for event in script.events() {
        offsets.push(acc);
        acc += event.render().len();
    }
    offsets
}

/// Drives a turn, collecting every delivered event; returns the sequence
/// and the complete-or-error outcome.
pub async fn drive(
    turn: &mut ResumableTurn,
) -> (
    Vec<ProviderEvent>,
    Result<CompletedTurn, ProviderStreamError>,
) {
    let mut seen = Vec::new();
    while let Some(result) = turn.next_event().await {
        match result {
            Ok(event) => seen.push(event),
            Err(err) => return (seen, Err(err)),
        }
    }
    (seen.clone(), turn.complete().await)
}

pub fn start(api: Arc<FakeApi>, plans: Vec<Plan>, max_resumes: u32) -> (FakeBridge, ResumableTurn) {
    let bridge = FakeBridge::new(api, identity(), plans);
    let turn = ResumableTurn::start(
        Arc::new(bridge.clone()),
        CompletionRequest::new(identity(), "find todos"),
        fast_config(max_resumes),
        jitter(),
    );
    (bridge, turn)
}
