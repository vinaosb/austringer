//! Live tier (`#[ignore]`, never in CI): runs ONLY when
//! `AUSTRINGER_OLLAMA_URL` names a live OpenAI-compatible server.
//!
//! Docker recipe (Windows PowerShell):
//! ```text
//! docker run -d --name austringer-ollama -p 11434:11434 ollama/ollama
//! docker exec austringer-ollama ollama pull qwen2.5:1.5b
//! $env:AUSTRINGER_OLLAMA_URL = "http://localhost:11434/v1"
//! cargo nextest run -p austringer-providers --run-ignored only ollama_live
//! ```
//! `AUSTRINGER_OLLAMA_MODEL` overrides the model id (default
//! `qwen2.5:1.5b`).
//!
//! What this tier adds over the cassette suites (adapter_matrix,
//! wire_surfaces): a REAL SSE stream with real server timing through
//! the production `ReqwestTransport` and `CompatAdapter`, and a real
//! server accepting the wire surfaces (system prompt + declared
//! tools). The cassettes pin bytes; this tier proves the live path.

use std::sync::Arc;

use austringer_core::tools::ToolSchema;
use austringer_providers::adapters::CompatAdapter;
use austringer_providers::auth::NoCredentials;
use austringer_providers::event::ProviderEvent;
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::provider::Provider;
use austringer_providers::transport_reqwest::ReqwestTransport;

mod common;

use common::wirekit;

fn live_base_url() -> String {
    std::env::var("AUSTRINGER_OLLAMA_URL").expect(
        "AUSTRINGER_OLLAMA_URL must name the live base URL (e.g. http://localhost:11434/v1); \
         docker recipe in this module's docs",
    )
}

fn live_model() -> String {
    std::env::var("AUSTRINGER_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:1.5b".to_owned())
}

fn live_adapter() -> CompatAdapter {
    let transport = ReqwestTransport::new().expect("reqwest transport builds");
    CompatAdapter::new(
        Arc::new(transport),
        Arc::new(NoCredentials),
        &live_base_url(),
        &live_model(),
    )
}

fn assembled_text(events: &[ProviderEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
#[ignore = "live tier: needs a local OpenAI-compatible server at AUSTRINGER_OLLAMA_URL (docker recipe in module docs); never CI"]
async fn live_compat_streams_deltas_and_assembles_text() {
    let adapter = live_adapter();
    let identity = ModelIdentity::new("openai-compat", &live_model());
    let stream = adapter
        .stream_completion(&CompletionRequest::new(
            identity,
            "Reply with exactly: PONG",
        ))
        .await
        .expect("live stream opens");
    let (events, terminal) = wirekit::collect(stream).await;
    assert!(terminal.is_none(), "stream must end cleanly: {terminal:?}");
    let text = assembled_text(&events);
    assert!(
        text.to_lowercase().contains("pong"),
        "assembled text should contain PONG, got {text:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProviderEvent::Usage { .. })),
        "usage event must arrive (server-side token accounting)"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProviderEvent::Finish { .. })),
        "finish event must arrive"
    );
}

#[tokio::test]
#[ignore = "live tier: needs a local OpenAI-compatible server at AUSTRINGER_OLLAMA_URL (docker recipe in module docs); never CI"]
async fn live_compat_serves_with_declared_tools() {
    let adapter = live_adapter();
    let identity = ModelIdentity::new("openai-compat", &live_model());
    let tools = vec![ToolSchema {
        name: "get_weather".to_owned(),
        description: "Get the current weather for a city.".to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        }),
    }];
    let request = CompletionRequest::new(
        identity,
        "What is the weather in Tokyo? Answer by calling the get_weather tool.",
    )
    .with_system("You are a helpful assistant that uses tools when offered.")
    .with_tools(tools);
    let stream = adapter
        .stream_completion(&request)
        .await
        .expect("live stream opens with declared tools");
    let (events, terminal) = wirekit::collect(stream).await;
    assert!(
        terminal.is_none(),
        "server must accept the system+tools surfaces: {terminal:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProviderEvent::Usage { .. })),
        "usage must be accounted for the tools-bearing request"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ProviderEvent::Finish { .. })),
        "finish event must arrive"
    );
    // If the model chose to call a tool, the adapter must surface the
    // declared name (never a mangled or unnamed call).
    for event in &events {
        if let ProviderEvent::ToolCallStart { name, .. } = event {
            assert_eq!(
                name, "get_weather",
                "tool call must carry the declared name"
            );
        }
    }
}
