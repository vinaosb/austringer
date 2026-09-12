//! The failure matrix, every adapter shape: malformed SSE (skipped + logged,
//! stream continues), 429-then-success (transparent retry), 500-exhaustion
//! (typed RetryExhausted), transport timeouts (retry-class engagement).
//! One row per adapter — anthropic, openai-chat, openai-responses, xai,
//! google, openai-compat.

use std::sync::Arc;

use harness_providers::adapters::{
    AnthropicAdapter, CompatAdapter, GoogleAdapter, OpenAiAdapter, XaiAdapter,
};
use harness_providers::auth::StaticApiKey;
use harness_providers::error::ProviderStreamError;
use harness_providers::event::{FinishReason, ProviderEvent};
use harness_providers::identity::ModelIdentity;
use harness_providers::provider::Provider;
use harness_providers::transport::TransportFailure;
use harness_test_support::sse::{SseEvent, SseScript};

use common::wirekit::{self, Chaos, ScriptedTransport, Staged};

mod common;

use common::frames::{
    anthropic_happy, chat_happy, gemini_happy, rs_args_delta, rs_completed, rs_item_added, rs_text,
};

type MakeAdapter = Box<dyn Fn(Arc<common::wirekit::ScriptedTransport>) -> Arc<dyn Provider>>;

struct Row {
    name: &'static str,
    identity: ModelIdentity,
    make: MakeAdapter,
    happy: fn() -> SseScript,
    expected_text: &'static str,
    expected_finish: FinishReason,
}

fn rows() -> Vec<Row> {
    let bearer = || Arc::new(StaticApiKey::bearer("sk-matrix"));
    vec![
        Row {
            name: "anthropic",
            identity: ModelIdentity::new("anthropic", "claude-sonnet-4-5"),
            make: Box::new(move |transport| {
                Arc::new(AnthropicAdapter::new(
                    transport,
                    bearer(),
                    "claude-sonnet-4-5",
                ))
            }),
            happy: || anthropic_happy("claude-sonnet-4-5"),
            expected_text: "Let me check the todos.",
            expected_finish: FinishReason::ToolCall,
        },
        Row {
            name: "openai-chat",
            identity: ModelIdentity::new("openai", "gpt-5.2"),
            make: Box::new(move |transport| {
                Arc::new(OpenAiAdapter::chat(transport, bearer(), "gpt-5.2"))
            }),
            happy: || chat_happy("gpt-5.2"),
            expected_text: "Checking the plan.",
            expected_finish: FinishReason::ToolCall,
        },
        Row {
            name: "openai-responses",
            identity: ModelIdentity::new("openai", "gpt-5.2"),
            make: Box::new(move |transport| {
                Arc::new(OpenAiAdapter::responses(transport, bearer(), "gpt-5.2"))
            }),
            happy: || responses_happy(),
            expected_text: "Checking logs.",
            expected_finish: FinishReason::ToolCall,
        },
        Row {
            name: "xai",
            identity: ModelIdentity::new("xai", "grok-4"),
            make: Box::new(move |transport| {
                Arc::new(XaiAdapter::new(transport, bearer(), "grok-4"))
            }),
            happy: || chat_happy("grok-4"),
            expected_text: "Checking the plan.",
            expected_finish: FinishReason::ToolCall,
        },
        Row {
            name: "google",
            identity: ModelIdentity::new("google", "gemini-2.5-flash"),
            make: Box::new(move |transport| {
                Arc::new(GoogleAdapter::new(
                    transport,
                    Arc::new(StaticApiKey::google_api_key("g")),
                    "gemini-2.5-flash",
                ))
            }),
            happy: gemini_happy,
            expected_text: "Reading the index.",
            expected_finish: FinishReason::ToolCall,
        },
        Row {
            name: "openai-compat",
            identity: ModelIdentity::new("openai-compat", "llama3.1:70b"),
            make: Box::new(move |transport| {
                Arc::new(CompatAdapter::new(
                    transport,
                    wirekit::no_auth(),
                    "http://localhost:11434/v1",
                    "llama3.1:70b",
                ))
            }),
            happy: || chat_happy("llama3.1:70b"),
            expected_text: "Checking the plan.",
            expected_finish: FinishReason::ToolCall,
        },
    ]
}

fn responses_happy() -> SseScript {
    harness_test_support::sse::SseScriptBuilder::new()
        .event(rs_text("Checking "))
        .event(rs_text("logs."))
        .event(rs_item_added("fc_01", "execute_command"))
        .event(rs_args_delta("fc_01", "{\"cmd\":\"ls\"}"))
        .event(rs_completed("gpt-5.2", 500, 25, 0))
        .build()
}

fn err_body(status: u16) -> serde_json::Value {
    serde_json::json!({ "error": { "type": "matrix", "code": status, "message": "matrix failure" } })
}

#[tokio::test]
async fn malformed_sse_frames_skip_and_stream_continues_per_adapter() {
    for row in rows() {
        let mut events = (row.happy)().events();
        let malformed_at = events.len() / 2;
        events.insert(
            malformed_at,
            SseEvent::new(
                "message",
                serde_json::Value::String("garbage-payload".to_owned()),
            ),
        );
        events.insert(
            malformed_at + 1,
            SseEvent::new("message", serde_json::json!({ "nonsense": [1, 2, 3] })),
        );
        let transport = ScriptedTransport::new(vec![Staged::Sse(
            SseScript::from_events(events),
            Chaos::None,
        )]);
        let provider = (row.make)(transport);
        let (delivered, terminal) = wirekit::collect(
            provider
                .stream_completion(&harness_providers::identity::CompletionRequest::new(
                    row.identity.clone(),
                    "p",
                ))
                .await
                .unwrap_or_else(|err| panic!("{}: open failed: {err}", row.name)),
        )
        .await;
        assert!(terminal.is_none(), "{}: terminal {terminal:?}", row.name);
        assert_eq!(
            wirekit::text_of(&delivered),
            row.expected_text,
            "{}",
            row.name
        );
        assert!(
            matches!(
                delivered.last(),
                Some(ProviderEvent::Finish { reason, .. }) if *reason == row.expected_finish
            ),
            "{}: finish missing or wrong after malformed frames",
            row.name
        );
    }
}

#[tokio::test]
async fn rate_limit_then_success_retries_transparently_per_adapter() {
    for row in rows() {
        let transport = ScriptedTransport::new(vec![
            Staged::HttpError(429, err_body(429)),
            Staged::sse((row.happy)()),
        ]);
        let provider = (row.make)(transport.clone());
        let (events, outcome) = wirekit::drive_turn(provider, row.identity.clone(), "p", 3).await;
        assert!(outcome.is_ok(), "{}: {outcome:?}", row.name);
        assert_eq!(wirekit::text_of(&events), row.expected_text, "{}", row.name);
        assert!(
            matches!(
                events.last(),
                Some(ProviderEvent::Finish { reason, .. }) if *reason == row.expected_finish
            ),
            "{}",
            row.name
        );
        assert_eq!(
            transport.requests().len(),
            2,
            "{}: exactly one retry after the 429",
            row.name
        );
    }
}

#[tokio::test]
async fn server_error_exhaustion_is_typed_per_adapter() {
    for row in rows() {
        let transport = ScriptedTransport::new(vec![
            Staged::HttpError(500, err_body(500)),
            Staged::HttpError(500, err_body(500)),
            Staged::HttpError(500, err_body(500)),
        ]);
        let provider = (row.make)(transport.clone());
        let (_, outcome) = wirekit::drive_turn(provider, row.identity.clone(), "p", 3).await;
        match outcome {
            Err(ProviderStreamError::RetryExhausted { attempts, last }) => {
                assert_eq!(attempts, 3, "{}", row.name);
                assert!(
                    matches!(*last, ProviderStreamError::Http { status: 500, .. }),
                    "{}: final cause {last:?}",
                    row.name
                );
            }
            other => panic!("{}: expected RetryExhausted, got {other:?}", row.name),
        }
        assert_eq!(transport.requests().len(), 3, "{}", row.name);
    }
}

#[tokio::test]
async fn transport_timeouts_engage_retry_policy_per_adapter() {
    for row in rows() {
        let transport = ScriptedTransport::new(vec![
            Staged::Transport(TransportFailure::Timeout(
                "matrix".to_owned()
            ));
            3
        ]);
        let provider = (row.make)(transport.clone());
        let (_, outcome) = wirekit::drive_turn(provider, row.identity.clone(), "p", 3).await;
        match outcome {
            Err(ProviderStreamError::RetryExhausted { attempts, last }) => {
                assert_eq!(attempts, 3, "{}", row.name);
                assert!(
                    matches!(
                        *last,
                        ProviderStreamError::Transport(TransportFailure::Timeout(_))
                    ),
                    "{}: final cause {last:?}",
                    row.name
                );
            }
            other => panic!("{}: expected RetryExhausted, got {other:?}", row.name),
        }
        assert_eq!(transport.requests().len(), 3, "{}", row.name);
    }
}
