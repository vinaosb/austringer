//! Google Gemini adapter: text parts, synthesized tool-call ids, finish
//! mapping (STOP→ToolCall when function calls streamed), usageMetadata.

use std::sync::Arc;

use harness_providers::adapters::GoogleAdapter;
use harness_providers::auth::StaticApiKey;
use harness_providers::event::{FinishReason, ProviderEvent};
use harness_providers::identity::{CompletionRequest, ModelIdentity};
use harness_providers::provider::Provider;
use harness_test_support::sse::{SseEvent, SseScriptBuilder};

use common::wirekit::{self, ScriptedTransport, Staged};

mod common;

use common::frames::{gemini_happy, gm_finish, gm_text, gm_usage};

fn google_key() -> Arc<StaticApiKey> {
    Arc::new(StaticApiKey::google_api_key("g-key"))
}

fn identity() -> ModelIdentity {
    ModelIdentity::new("google", "gemini-2.5-flash")
}

#[tokio::test]
async fn happy_path_synthesizes_stable_call_ids_and_maps_usage() {
    let transport = ScriptedTransport::new(vec![Staged::sse(gemini_happy())]);
    let adapter = GoogleAdapter::new(transport.clone(), google_key(), "gemini-2.5-flash");
    let (events, terminal) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity(), "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(terminal.is_none());
    assert_eq!(wirekit::text_of(&events), "Reading the index.");
    let start = events.iter().find_map(|event| match event {
        ProviderEvent::ToolCallStart { call_id, name, .. } => Some((call_id.clone(), name.clone())),
        _ => None,
    });
    assert_eq!(
        start,
        Some(("gemini-call-0".to_owned(), "tool_search".to_owned()))
    );
    let args = events
        .iter()
        .filter_map(|event| match event {
            ProviderEvent::ToolCallDelta {
                call_id,
                arguments_fragment,
                ..
            } if call_id == "gemini-call-0" => Some(arguments_fragment.clone()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(args, "{\"query\":\"edit\"}");
    let usage = wirekit::usage_of(&events);
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens
        ),
        (777, 55, 100)
    );
    assert!(matches!(
        events.last(),
        Some(ProviderEvent::Finish {
            reason: FinishReason::ToolCall,
            ..
        })
    ));
    let sent = transport.requests();
    assert_eq!(
        sent[0].url,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse"
    );
    assert_eq!(
        sent[0].headers.get("x-goog-api-key").map(String::as_str),
        Some("g-key")
    );
    assert_eq!(
        transport.body_json(0)["contents"][0]["parts"][0]["text"],
        "p"
    );
}

#[tokio::test]
async fn plain_text_stop_maps_to_end_turn() {
    let script = SseScriptBuilder::new()
        .event(gm_text("done"))
        .event(gm_usage(10, 2, 0))
        .event(gm_finish("STOP"))
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = GoogleAdapter::new(transport, google_key(), "gemini-2.5-flash");
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
async fn safety_and_length_finish_reasons_map_to_the_vocabulary() {
    for (wire, expected) in [
        ("MAX_TOKENS", FinishReason::Length),
        ("SAFETY", FinishReason::Refusal),
    ] {
        let script = SseScriptBuilder::new()
            .event(gm_text("x"))
            .event(gm_finish(wire))
            .build();
        let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
        let adapter = GoogleAdapter::new(transport, google_key(), "gemini-2.5-flash");
        let (events, _) = wirekit::collect(
            adapter
                .stream_completion(&CompletionRequest::new(identity(), "p"))
                .await
                .expect("open"),
        )
        .await;
        assert_eq!(
            events.last(),
            Some(&ProviderEvent::Finish {
                model: identity(),
                reason: expected,
            }),
            "finishReason {wire}"
        );
    }
}

#[tokio::test]
async fn gemini_error_shape_maps_to_typed_http_error() {
    let transport = ScriptedTransport::new(vec![Staged::HttpError(
        429,
        serde_json::json!({ "error": { "code": 429, "message": "quota exceeded", "status": "RESOURCE_EXHAUSTED" } }),
    )]);
    let adapter = GoogleAdapter::new(transport, google_key(), "gemini-2.5-flash");
    let verdict = adapter
        .stream_completion(&CompletionRequest::new(identity(), "p"))
        .await;
    let err = match verdict {
        Err(err) => err,
        Ok(stream) => {
            drop(stream);
            panic!("429 must surface as a typed error before any stream opens")
        }
    };
    assert!(
        matches!(
            &err,
            harness_providers::error::ProviderStreamError::Http { status, code, .. }
                if *status == 429 && code == "RESOURCE_EXHAUSTED"
        ),
        "got: {err:?}"
    );
    assert!(err.is_recoverable(), "429 must classify retryable");
}

#[tokio::test]
async fn unknown_fields_and_null_parts_are_tolerated() {
    let script = SseScriptBuilder::new()
        .event(SseEvent::new(
            "message",
            serde_json::json!({
                "candidates": [{ "content": { "parts": [{ "text": "ok", "futureField": 1 }] }, "finishReason": "STOP" }],
                "usageMetadata": { "promptTokenCount": 5, "candidatesTokenCount": 1, "thoughtsTokenCount": 99 },
                "modelVersion": "gemini-2.5-flash",
                "unknownTop": { "nested": true }
            }),
        ))
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = GoogleAdapter::new(transport, google_key(), "gemini-2.5-flash");
    let (events, terminal) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity(), "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(terminal.is_none());
    assert_eq!(wirekit::text_of(&events), "ok");
    assert_eq!(events[0].model().model, "gemini-2.5-flash");
}
