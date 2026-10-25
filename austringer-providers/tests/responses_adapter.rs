//! OpenAI Responses API: output_text deltas, function_call items with
//! argument deltas, completed/incomplete/failed terminal events.

use std::sync::Arc;

use austringer_providers::adapters::OpenAiAdapter;
use austringer_providers::auth::StaticApiKey;
use austringer_providers::event::{FinishReason, ProviderEvent};
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::provider::Provider;
use austringer_test_support::sse::SseScriptBuilder;

use common::wirekit::{self, ScriptedTransport, Staged};

mod common;

use common::frames::{
    rs_args_delta, rs_completed, rs_failed, rs_incomplete, rs_item_added, rs_text,
};

fn bearer() -> Arc<StaticApiKey> {
    Arc::new(StaticApiKey::bearer("sk-oai"))
}

fn identity() -> ModelIdentity {
    ModelIdentity::new("openai", "gpt-5.2")
}

#[tokio::test]
async fn responses_happy_path_maps_items_deltas_and_completed() {
    let script = SseScriptBuilder::new()
        .event(austringer_test_support::sse::SseEvent::new(
            "response.created",
            serde_json::json!({ "response": { "id": "resp_1" } }),
        ))
        .event(rs_text("Checking "))
        .event(rs_text("logs."))
        .event(rs_item_added("fc_01", "execute_command"))
        .event(rs_args_delta("fc_01", "{\"cmd\":"))
        .event(rs_args_delta("fc_01", "\"ls\"}"))
        .event(rs_completed("gpt-5.2", 500, 25, 200))
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = OpenAiAdapter::responses(transport.clone(), bearer(), "gpt-5.2");
    let (events, terminal) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity(), "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(terminal.is_none());
    assert_eq!(wirekit::text_of(&events), "Checking logs.");
    let usage = wirekit::usage_of(&events);
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens
        ),
        (500, 25, 200)
    );
    let finish = events.last();
    assert_eq!(
        finish,
        Some(&ProviderEvent::Finish {
            model: identity(),
            reason: FinishReason::ToolCall,
        })
    );
    let sent = transport.requests();
    assert_eq!(sent[0].url, "https://api.openai.com/v1/responses");
    assert_eq!(transport.body_json(0)["input"], "p");
}

#[tokio::test]
async fn plain_text_completion_maps_to_end_turn() {
    let script = SseScriptBuilder::new()
        .event(rs_text("ok"))
        .event(rs_completed("gpt-5.2", 5, 1, 0))
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = OpenAiAdapter::responses(transport, bearer(), "gpt-5.2");
    let (events, _) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity(), "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(matches!(
        events.last(),
        Some(ProviderEvent::Finish {
            reason: FinishReason::EndTurn,
            ..
        })
    ));
}

#[tokio::test]
async fn incomplete_maps_to_length_and_failed_to_stream_error() {
    let script = SseScriptBuilder::new()
        .event(rs_text("partial"))
        .event(rs_incomplete("max_output_tokens"))
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = OpenAiAdapter::responses(transport, bearer(), "gpt-5.2");
    let (events, _) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity(), "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(matches!(
        events.last(),
        Some(ProviderEvent::Finish {
            reason: FinishReason::Length,
            ..
        })
    ));

    let script = SseScriptBuilder::new()
        .event(rs_failed("server_error", "upstream broke"))
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = OpenAiAdapter::responses(transport, bearer(), "gpt-5.2");
    let (events, terminal) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity(), "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(
        matches!(
            terminal,
            Some(austringer_providers::error::ProviderStreamError::StreamError { ref code, .. }) if code == "server_error"
        ),
        "terminal: {terminal:?}"
    );
    assert!(events.is_empty());
}
