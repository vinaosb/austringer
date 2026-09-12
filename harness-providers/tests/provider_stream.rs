//! Provider-trait tests: dyn-safe streaming over the T2 fake, the happy
//! 200-event path (QA happy channel), and request mapping onto the T2
//! `ProviderRequest` shape.

mod common;

use std::sync::Arc;

use common::{FakeBridge, Plan, n_event_script};
use harness_providers::event::{FinishReason, ProviderEvent, Usage};
use harness_providers::identity::{CompletionRequest, ModelIdentity};
use harness_providers::provider::Provider;

fn identity() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-4-5")
}

#[tokio::test]
async fn two_hundred_event_stream_completes_through_dyn_provider() {
    let api = Arc::new(harness_test_support::provider::FakeApi::new());
    let bridge = FakeBridge::new(api, identity(), vec![Plan::ok(n_event_script(200))]);
    // Dyn-safety: the whole run goes through the trait object; the concrete
    // bridge is kept only for the served-request assertions below.
    let provider: Arc<dyn Provider> = Arc::new(bridge.clone());
    let request = CompletionRequest::new(identity(), "count to two hundred");
    let mut stream = provider
        .stream_completion(&request)
        .await
        .expect("stream opens");

    let mut texts = Vec::new();
    let mut usage = None;
    let mut finish = None;
    let mut events = 0;
    while let Some(result) = stream.next().await {
        let event = result.expect("no error in happy path");
        events += 1;
        match event {
            ProviderEvent::TextDelta { model, text } => {
                assert_eq!(model, identity());
                texts.push(text);
            }
            ProviderEvent::Usage { usage: u, .. } => usage = Some(u),
            ProviderEvent::Finish { reason, .. } => finish = Some(reason),
            ProviderEvent::ToolCallStart { .. } | ProviderEvent::ToolCallDelta { .. } => {
                panic!("no tool calls in this script")
            }
        }
    }

    // QA happy evidence: event count, assembled text, usage.
    assert_eq!(events, 202, "200 deltas + usage + finish");
    assert_eq!(
        texts.concat(),
        (0..200).map(|i| format!("tok{i} ")).collect::<String>()
    );
    assert_eq!(
        usage,
        Some(Usage {
            input_tokens: 120,
            output_tokens: 200
        })
    );
    assert_eq!(finish, Some(FinishReason::EndTurn));

    // The request reached the fake mapped ONTO the T2 shape (model + prompt).
    let served = bridge.served();
    assert_eq!(served.len(), 1);
    assert_eq!(served[0].model, "claude-4-5");
    assert_eq!(served[0].prompt, "count to two hundred");
}
