//! Chat-completions family: OpenAI, xAI, and the OpenAI-compatible adapter.
//! One codec, three endpoints — indexed tool fragments, deferred finish
//! (usage arrives after `finish_reason`), and [DONE].

use std::sync::Arc;

use harness_providers::adapters::{CompatAdapter, OpenAiAdapter, XaiAdapter};
use harness_providers::auth::StaticApiKey;
use harness_providers::event::{FinishReason, ProviderEvent};
use harness_providers::identity::{CompletionRequest, ModelIdentity};
use harness_providers::provider::Provider;

use common::wirekit::{self, ScriptedTransport, Staged};

mod common;

use common::frames::{cc_done, cc_finish, cc_text, cc_tool_args, cc_tool_start, chat_happy};

fn bearer() -> Arc<StaticApiKey> {
    Arc::new(StaticApiKey::bearer("sk-oai"))
}

#[tokio::test]
async fn openai_happy_path_orders_usage_before_deferred_finish() {
    let transport = ScriptedTransport::new(vec![Staged::sse(chat_happy("gpt-5.2"))]);
    let adapter = OpenAiAdapter::chat(transport.clone(), bearer(), "gpt-5.2");
    let identity = ModelIdentity::new("openai", "gpt-5.2");
    let (events, terminal) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity, "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(terminal.is_none());
    let usage_pos = events
        .iter()
        .position(|event| matches!(event, ProviderEvent::Usage { .. }))
        .expect("usage event");
    let finish_pos = events
        .iter()
        .position(|event| matches!(event, ProviderEvent::Finish { .. }))
        .expect("finish event");
    assert!(
        usage_pos < finish_pos,
        "usage must precede the buffered finish"
    );
    assert_eq!(finish_pos, events.len() - 1, "finish is terminal");
    let usage = wirekit::usage_of(&events);
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens
        ),
        (900, 33, 400)
    );
    assert!(matches!(
        events[finish_pos],
        ProviderEvent::Finish {
            reason: FinishReason::ToolCall,
            ..
        }
    ));
    let sent = transport.requests();
    assert_eq!(sent[0].url, "https://api.openai.com/v1/chat/completions");
    assert_eq!(
        sent[0].headers.get("authorization").map(String::as_str),
        Some("Bearer sk-oai")
    );
    let body = transport.body_json(0);
    assert_eq!(body["model"], "gpt-5.2");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["messages"][0]["content"], "p");
}

#[tokio::test]
async fn tool_fragments_correlate_by_index_across_id_first_fragment() {
    let script = harness_test_support::sse::SseScriptBuilder::new()
        .event(cc_text("gpt-5.2", "plan"))
        .event(cc_tool_start("gpt-5.2", 3, "call_zz", "edit_lines"))
        .event(cc_tool_args("gpt-5.2", 3, "{\"path\":"))
        .event(cc_tool_args("gpt-5.2", 3, "\"x\"}"))
        .event(cc_finish("gpt-5.2", "tool_calls"))
        .event(cc_done())
        .build();
    let transport = ScriptedTransport::new(vec![Staged::sse(script)]);
    let adapter = OpenAiAdapter::chat(transport, bearer(), "gpt-5.2");
    let identity = ModelIdentity::new("openai", "gpt-5.2");
    let (events, _) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity, "p"))
            .await
            .expect("open"),
    )
    .await;
    let mut args = String::new();
    let mut call_ids = Vec::new();
    for event in &events {
        match event {
            ProviderEvent::ToolCallStart { call_id, name, .. } => {
                assert_eq!(name, "edit_lines");
                call_ids.push(call_id.clone());
            }
            ProviderEvent::ToolCallDelta {
                call_id,
                arguments_fragment,
                ..
            } => {
                assert_eq!(call_id, "call_zz", "later fragments follow the index map");
                args.push_str(arguments_fragment);
            }
            _ => {}
        }
    }
    assert_eq!(call_ids, vec!["call_zz"]);
    assert_eq!(args, "{\"path\":\"x\"}");
}

#[tokio::test]
async fn xai_targets_its_own_endpoint() {
    let transport = ScriptedTransport::new(vec![Staged::sse(chat_happy("grok-4"))]);
    let adapter = XaiAdapter::new(transport.clone(), bearer(), "grok-4");
    let identity = ModelIdentity::new("xai", "grok-4");
    let stream = adapter
        .stream_completion(&CompletionRequest::new(identity, "p"))
        .await
        .expect("open");
    drop(stream);
    let sent = transport.requests();
    assert_eq!(sent[0].url, "https://api.x.ai/v1/chat/completions");
    assert_eq!(transport.body_json(0)["model"], "grok-4");
}

#[tokio::test]
async fn compat_targets_configured_base_and_allows_no_auth() {
    let transport = ScriptedTransport::new(vec![Staged::sse(chat_happy("llama3.1:70b"))]);
    let adapter = CompatAdapter::new(
        transport.clone(),
        wirekit::no_auth(),
        "http://localhost:11434/v1",
        "llama3.1:70b",
    );
    let identity = ModelIdentity::new("openai-compat", "llama3.1:70b");
    let (events, terminal) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity, "p"))
            .await
            .expect("open"),
    )
    .await;
    assert!(terminal.is_none());
    let sent = transport.requests();
    assert_eq!(sent[0].url, "http://localhost:11434/v1/chat/completions");
    assert!(
        !sent[0].headers.contains_key("authorization")
            && !sent[0].headers.contains_key("x-api-key"),
        "local endpoints carry no credentials"
    );
    assert_eq!(wirekit::text_of(&events), "Checking the plan.");
}

#[tokio::test]
async fn compat_unknown_model_is_unpriced_tokens_only() {
    let transport = ScriptedTransport::new(vec![Staged::sse(chat_happy("local-custom-70b"))]);
    let adapter = CompatAdapter::new(
        transport,
        wirekit::no_auth(),
        "http://localhost:11434/v1",
        "local-custom-70b",
    );
    let identity = ModelIdentity::new("openai-compat", "local-custom-70b");
    let (events, _) = wirekit::collect(
        adapter
            .stream_completion(&CompletionRequest::new(identity.clone(), "p"))
            .await
            .expect("open"),
    )
    .await;
    let report = harness_providers::cost::CostTable::builtin()
        .account(&identity, wirekit::usage_of(&events));
    assert_eq!(report.priced, harness_providers::cost::Priced::Unpriced);
    assert_eq!(report.usage.input_tokens, 900);
}
