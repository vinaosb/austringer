//! Anthropic adapter: request rendering, happy-path streaming with tool
//! calls and cache-aware usage, auth surfaces, and wire-model stamping.

use std::sync::Arc;

use austringer_providers::auth::{AuthKind, StaticApiKey};
use austringer_providers::cost::{CostTable, Priced};
use austringer_providers::event::{FinishReason, ProviderEvent};
use austringer_providers::identity::ModelIdentity;
use austringer_providers::provider::Provider;
use austringer_test_support::sse::SseScriptBuilder;

use common::wirekit::{self, ScriptedTransport, Staged};

mod common;

use common::frames::{an_message_delta, an_text_delta, anthropic_happy};

fn identity() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-sonnet-4-5")
}

fn anthropic_key() -> Arc<StaticApiKey> {
    Arc::new(StaticApiKey::x_api_key("sk-ant-test"))
}

#[tokio::test]
async fn happy_path_streams_text_tool_usage_and_finish_in_order() {
    let transport = ScriptedTransport::new(vec![Staged::sse(anthropic_happy("claude-sonnet-4-5"))]);
    let adapter = austringer_providers::adapters::AnthropicAdapter::new(
        transport.clone(),
        anthropic_key(),
        "claude-sonnet-4-5",
    );
    let request = austringer_providers::identity::CompletionRequest::new(identity(), "find todos");
    let stream = adapter
        .stream_completion(&request)
        .await
        .expect("adapter opens the scripted stream");
    let (events, terminal) = wirekit::collect(stream).await;
    assert!(
        terminal.is_none(),
        "happy path must not error: {terminal:?}"
    );

    let kinds: Vec<&str> = events
        .iter()
        .map(|event| match event {
            ProviderEvent::TextDelta { .. } => "text",
            ProviderEvent::ToolCallStart { .. } => "tool-start",
            ProviderEvent::ToolCallDelta { .. } => "tool-delta",
            ProviderEvent::Usage { .. } => "usage",
            ProviderEvent::Finish { .. } => "finish",
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "text",
            "text",
            "tool-start",
            "tool-delta",
            "tool-delta",
            "usage",
            "finish"
        ]
    );
    assert_eq!(wirekit::text_of(&events), "Let me check the todos.");
    let (call_id, name) = events
        .iter()
        .find_map(|event| match event {
            ProviderEvent::ToolCallStart { call_id, name, .. } => {
                Some((call_id.clone(), name.clone()))
            }
            _ => None,
        })
        .unwrap_or_default();
    assert_eq!(call_id, "toolu_01");
    assert_eq!(name, "execute_command");
    let usage = wirekit::usage_of(&events);
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens,
            usage.cache_write_tokens
        ),
        (1_234, 42, 5_000, 1_000)
    );
    assert!(matches!(
        events.last(),
        Some(ProviderEvent::Finish {
            reason: FinishReason::ToolCall,
            ..
        })
    ));
}

#[tokio::test]
async fn rendered_request_carries_wire_body_and_headers() {
    let transport = ScriptedTransport::new(vec![Staged::sse(anthropic_happy("claude-sonnet-4-5"))]);
    let adapter = austringer_providers::adapters::AnthropicAdapter::new(
        transport.clone(),
        anthropic_key(),
        "claude-sonnet-4-5",
    );
    let request = austringer_providers::identity::CompletionRequest::new(identity(), "find todos");
    let stream = adapter.stream_completion(&request).await.expect("open");
    drop(stream);

    let sent = transport.requests();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].url, "https://api.anthropic.com/v1/messages");
    assert_eq!(
        sent[0].headers.get("x-api-key").map(String::as_str),
        Some("sk-ant-test")
    );
    assert_eq!(
        sent[0].headers.get("anthropic-version").map(String::as_str),
        Some("2023-06-01")
    );
    assert_eq!(
        sent[0].headers.get("accept").map(String::as_str),
        Some("text/event-stream")
    );
    let body = transport.body_json(0);
    assert_eq!(body["model"], "claude-sonnet-4-5");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 4096);
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "find todos");
}

#[tokio::test]
async fn cost_accounting_from_the_builtin_table_matches_the_usage_event() {
    let transport = ScriptedTransport::new(vec![Staged::sse(anthropic_happy("claude-sonnet-4-5"))]);
    let adapter = austringer_providers::adapters::AnthropicAdapter::new(
        transport,
        anthropic_key(),
        "claude-sonnet-4-5",
    );
    let request = austringer_providers::identity::CompletionRequest::new(identity(), "find todos");
    let (events, _) =
        wirekit::collect(adapter.stream_completion(&request).await.expect("open")).await;
    let report = CostTable::builtin().account(&identity(), wirekit::usage_of(&events));
    let Priced::Priced(costs) = report.priced else {
        panic!("claude-sonnet-4-5 is priced");
    };
    // (1234*3 + 42*15 + 5000*0.3 + 1000*3.75) / 1e6 USD in micros.
    assert_eq!(costs.total_usd_micros, 3_702 + 630 + 1_500 + 3_750);
}

#[tokio::test]
async fn served_model_from_the_wire_wins_identity_stamping() {
    // The stream claims a different model than configured: the events carry
    // the SERVED identity, which the turn layer rejects as a mismatch.
    let transport = ScriptedTransport::new(vec![Staged::sse(anthropic_happy("claude-opus-4-5"))]);
    let adapter = austringer_providers::adapters::AnthropicAdapter::new(
        transport,
        anthropic_key(),
        "claude-sonnet-4-5",
    );
    let request = austringer_providers::identity::CompletionRequest::new(identity(), "p");
    let (events, _) =
        wirekit::collect(adapter.stream_completion(&request).await.expect("open")).await;
    assert_eq!(events[0].model().model, "claude-opus-4-5");
}

#[tokio::test]
async fn bearer_style_key_sends_authorization_header_and_kind_is_api() {
    let transport = ScriptedTransport::new(vec![Staged::sse(anthropic_happy("claude-sonnet-4-5"))]);
    let auth = Arc::new(StaticApiKey::bearer("sub-token"));
    let adapter = austringer_providers::adapters::AnthropicAdapter::new(
        transport.clone(),
        auth,
        "claude-sonnet-4-5",
    );
    // A static bearer KEY is still metered API billing; Subscription kind is
    // carried only by OAuthDeviceFlow-backed sources (auth_flows suite).
    assert_eq!(adapter.auth_kind(), AuthKind::ApiKey);
    let request = austringer_providers::identity::CompletionRequest::new(identity(), "p");
    drop(adapter.stream_completion(&request).await.expect("open"));
    let headers = &transport.requests()[0].headers;
    assert_eq!(
        headers.get("authorization").map(String::as_str),
        Some("Bearer sub-token")
    );
    assert!(headers.get("x-api-key").is_none());
}

#[tokio::test]
async fn weird_but_valid_payloads_stream_cleanly() {
    // Extra unknown fields, null optionals, unicode: ignored, not fatal.
    let script = SseScriptBuilder::new()
        .event(austringer_test_support::sse::SseEvent::new(
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "beta_field": { "nested": true },
                "message": { "model": "claude-sonnet-4-5", "usage": { "input_tokens": 9, "cache_read_input_tokens": serde_json::Value::Null } }
            }),
        ))
        .event(an_text_delta("héllo → 世界"))
        .event(an_message_delta(3, "end_turn"))
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = austringer_providers::adapters::AnthropicAdapter::new(
        transport,
        anthropic_key(),
        "claude-sonnet-4-5",
    );
    let request = austringer_providers::identity::CompletionRequest::new(identity(), "p");
    let (events, terminal) =
        wirekit::collect(adapter.stream_completion(&request).await.expect("open")).await;
    assert!(terminal.is_none());
    assert_eq!(wirekit::text_of(&events), "héllo → 世界");
    let usage = wirekit::usage_of(&events);
    assert_eq!(usage.input_tokens, 9);
    assert_eq!(usage.cache_read_tokens, 0, "null optionals read as zero");
}
