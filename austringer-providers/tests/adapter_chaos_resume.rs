//! Cancel/resume chaos through REAL adapters (adapter stream mid-drop →
//! T9 `ResumableTurn` exactly-once path). One test per adapter shape
//! minimum: Anthropic (named-events wire) + OpenAI-compat (data-only wire,
//! deferred finish).
//!
//! The first reply is torn mid-stream after the tool call's argument
//! fragments but before the finish; the resume re-issues the request, the
//! replayed stream is reconciled by offset + the dedup registry, and the
//! tool side effect executes exactly once.

use std::sync::Arc;
use std::time::Duration;

use austringer_providers::adapters::{AnthropicAdapter, CompatAdapter};
use austringer_providers::auth::StaticApiKey;
use austringer_providers::event::ProviderEvent;
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::retry::{FixedJitter, RetryPolicy};
use austringer_providers::turn::{RecoveryConfig, ResumableTurn};

use common::wirekit::{self, Chaos, ScriptedTransport, Staged};

mod common;

use common::frames::{anthropic_happy, chat_happy};

fn fast_recovery() -> RecoveryConfig {
    RecoveryConfig {
        retry: RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(4),
            multiplier: 2.0,
        },
        max_resumes: 4,
        idle_timeout: Duration::from_secs(60),
    }
}

/// Drives the turn, marking `call_id` executed as soon as its arguments
/// complete — the tool layer's exactly-once obligation.
async fn drive_exactly_once(turn: &mut ResumableTurn, executed_call: &str) -> Vec<ProviderEvent> {
    let mut seen = Vec::new();
    let mut executed = false;
    while let Some(item) = turn.next_event().await {
        let event = item.unwrap_or_else(|err| panic!("turn failed: {err:?}"));
        if matches!(
            &event,
            ProviderEvent::ToolCallDelta { call_id, arguments_fragment, .. }
                if call_id == executed_call && arguments_fragment.ends_with('}')
        ) && !executed
        {
            turn.mark_executed(executed_call);
            executed = true;
        }
        seen.push(event);
    }
    assert!(
        executed,
        "the tool call must have assembled before the drop"
    );
    seen
}

fn bytes_through(script: &austringer_test_support::sse::SseScript, frames: usize) -> u64 {
    script
        .events()
        .iter()
        .take(frames)
        .map(|event| event.render().len() as u64)
        .sum()
}

#[tokio::test]
async fn anthropic_mid_drop_resumes_exactly_once() {
    let happy = anthropic_happy("claude-sonnet-4-5");
    // 9 frames: start, text, text, tool_start, args, args, msg_delta, ping, stop.
    // Tear after frame 6 (args complete, no finish).
    let torn = bytes_through(&happy, 6);
    let transport = ScriptedTransport::new(vec![
        Staged::Sse(happy.clone(), Chaos::DropAfterBytes(torn)),
        Staged::sse(happy),
    ]);
    let adapter = AnthropicAdapter::new(
        transport.clone(),
        Arc::new(StaticApiKey::x_api_key("sk-ant")),
        "claude-sonnet-4-5",
    );
    let identity = ModelIdentity::new("anthropic", "claude-sonnet-4-5");
    let mut turn = ResumableTurn::start(
        Arc::new(adapter),
        CompletionRequest::new(identity.clone(), "find todos"),
        fast_recovery(),
        Box::new(FixedJitter::new([0.5])),
    );

    let seen = drive_exactly_once(&mut turn, "toolu_01").await;
    assert_eq!(wirekit::text_of(&seen), "Let me check the todos.");

    let completed = turn.complete().await.expect("resume completes the turn");
    assert_eq!(
        completed.tool_calls.len(),
        1,
        "tool call assembled exactly once"
    );
    assert_eq!(completed.tool_calls[0].call_id, "toolu_01");
    assert_eq!(
        completed.tool_calls[0].arguments,
        serde_json::json!({ "cmd": "grep -r TODO src/" })
    );
    assert_eq!(completed.usage.map(|usage| usage.output_tokens), Some(42));
    assert!(turn.dedup().contains("toolu_01"));
    assert_eq!(
        transport.requests().len(),
        2,
        "one re-issue, byte-identical (cassette-strict transport would reject otherwise)"
    );
    let bodies: Vec<String> = transport
        .requests()
        .iter()
        .map(|request| String::from_utf8_lossy(&request.body).to_string())
        .collect();
    assert_eq!(
        bodies[0], bodies[1],
        "resume re-issues the request verbatim"
    );
}

#[tokio::test]
async fn compat_mid_drop_resumes_exactly_once_with_deferred_finish() {
    let happy = chat_happy("llama3.1:70b");
    // 8 frames: text, text, tool_start, args, args, finish, usage, [DONE].
    // Tear after frame 5 (args complete, before the buffered finish).
    let torn = bytes_through(&happy, 5);
    let transport = ScriptedTransport::new(vec![
        Staged::Sse(happy.clone(), Chaos::DropAfterBytes(torn)),
        Staged::sse(happy),
    ]);
    let adapter = CompatAdapter::new(
        transport.clone(),
        wirekit::no_auth(),
        "http://localhost:11434/v1",
        "llama3.1:70b",
    );
    let identity = ModelIdentity::new("openai-compat", "llama3.1:70b");
    let mut turn = ResumableTurn::start(
        Arc::new(adapter),
        CompletionRequest::new(identity, "p"),
        fast_recovery(),
        Box::new(FixedJitter::new([0.5])),
    );

    let seen = drive_exactly_once(&mut turn, "call_abc").await;
    assert_eq!(wirekit::text_of(&seen), "Checking the plan.");

    let completed = turn.complete().await.expect("resume completes");
    assert_eq!(completed.tool_calls.len(), 1);
    assert_eq!(completed.tool_calls[0].call_id, "call_abc");
    assert_eq!(
        completed.tool_calls[0].arguments,
        serde_json::json!({ "path": "src/lib.rs" })
    );
    assert_eq!(completed.usage.map(|usage| usage.input_tokens), Some(900));
    assert_eq!(
        austringer_providers::event::FinishReason::ToolCall,
        completed.finish_reason,
        "replayed finish flushes after usage on the resumed stream"
    );
    assert_eq!(transport.requests().len(), 2);
}
