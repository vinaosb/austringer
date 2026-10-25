//! Shared kit for Inference-adapter tests: config, adapter construction,
//! and the recorded-event fixtures.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use super::{FakeBridge, Plan};
use austringer_core::event::{ContentBlock, Event, RecordedEvent, Sequence};
use austringer_providers::adapter::InferenceAdapter;
use austringer_providers::identity::ModelIdentity;
use austringer_providers::turn::RecoveryConfig;
use austringer_test_support::provider::FakeApi;

pub fn identity() -> ModelIdentity {
    ModelIdentity::new("anthropic", "claude-4-5")
}

pub fn config() -> RecoveryConfig {
    RecoveryConfig {
        retry: austringer_providers::retry::RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(4),
            multiplier: 2.0,
        },
        max_resumes: 4,
        idle_timeout: Duration::from_secs(60),
    }
}

pub fn adapter(plans: Vec<Plan>) -> (FakeBridge, InferenceAdapter) {
    let bridge = FakeBridge::new(Arc::new(FakeApi::new()), identity(), plans);
    let adapter = InferenceAdapter::new(Arc::new(bridge.clone()), config(), 99);
    (bridge, adapter)
}

pub fn recorded(seq: u64, event: Event) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event,
    }
}

pub fn text_block(text: &str) -> ContentBlock {
    ContentBlock::Text {
        text: text.to_owned(),
    }
}

pub fn context() -> Vec<RecordedEvent> {
    vec![
        recorded(
            1,
            Event::User {
                blocks: vec![text_block("find the todos")],
            },
        ),
        recorded(
            2,
            Event::Assistant {
                blocks: vec![text_block("on it")],
            },
        ),
        recorded(
            3,
            Event::ToolCall {
                call_id: "prev1".to_owned(),
                tool: "view_window".to_owned(),
                arguments: serde_json::json!({ "path": "x" }),
            },
        ),
        recorded(
            4,
            Event::ToolResult {
                call_id: "prev1".to_owned(),
                blocks: vec![text_block("ok")],
            },
        ),
        recorded(
            5,
            Event::System {
                text: "runtime notice".to_owned(),
            },
        ),
    ]
}
