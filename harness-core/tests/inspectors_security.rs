//! Security inspector tests (todo 7): the optional LLM risk reviewer is
//! off by default and fully mock-injected (no live API), marker calls
//! route to review (tool name and nested argument strings), reviewer
//! denials block with feedback, and a hung reviewer times out and fails
//! closed — probed deterministically under `start_paused`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use harness_core::agent_loop::PendingCall;
use harness_core::inspectors::chain::{ChainOutcome, InspectorChain, InspectorConfig};
use harness_core::inspectors::permission::{Effect, Rule};
use harness_core::inspectors::security::{
    ReviewOutcome, RiskReviewer, SecurityConfig, SecurityInspector,
};
use harness_core::inspectors::verdict::{CallInspector, Verdict};

fn call(tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: format!("call-{tool}"),
        tool: tool.to_owned(),
        arguments,
    }
}

fn risky_config() -> SecurityConfig {
    SecurityConfig {
        enabled: true,
        markers: vec!["dangerous".to_owned()],
        review_timeout: Duration::from_secs(5),
    }
}

/// Scripted reviewer: records every reviewed call, answers from a queue.
#[derive(Clone, Default)]
struct ReviewRecorder {
    reviewed: Arc<Mutex<Vec<PendingCall>>>,
    queue: Arc<Mutex<VecDeque<ReviewOutcome>>>,
}

impl ReviewRecorder {
    fn with(outcomes: Vec<ReviewOutcome>) -> Self {
        Self {
            reviewed: Arc::default(),
            queue: Arc::new(Mutex::new(outcomes.into_iter().collect())),
        }
    }

    fn reviewed(&self) -> usize {
        self.reviewed.lock().expect("reviewed").len()
    }
}

#[async_trait]
impl RiskReviewer for ReviewRecorder {
    async fn review(&mut self, call: &PendingCall) -> ReviewOutcome {
        self.reviewed.lock().expect("reviewed").push(call.clone());
        self.queue
            .lock()
            .expect("queue")
            .pop_front()
            .unwrap_or(ReviewOutcome::Approve)
    }
}

/// Never returns in time: models a hung reviewer for the timeout probe.
struct HungReviewer;

#[async_trait]
impl RiskReviewer for HungReviewer {
    async fn review(&mut self, _call: &PendingCall) -> ReviewOutcome {
        tokio::time::sleep(Duration::from_secs(10)).await;
        ReviewOutcome::Approve
    }
}

#[test]
fn security_is_off_by_default() {
    assert!(
        !SecurityConfig::default().enabled,
        "the LLM risk reviewer must be opt-in"
    );
}

#[tokio::test]
async fn disabled_inspector_allows_even_risky_looking_calls() {
    let mut inspector = SecurityInspector::new(SecurityConfig::default());
    assert_eq!(
        inspector
            .inspect(&call(
                "execute_command",
                serde_json::json!({"cmd": "dangerous wipe"})
            ))
            .await
            .expect("inspect"),
        Verdict::Allow {
            authoritative: false
        }
    );
}

#[tokio::test]
async fn marker_calls_route_to_the_reviewer() {
    let reviewer = ReviewRecorder::with(vec![ReviewOutcome::Approve]);
    let mut inspector =
        SecurityInspector::with_reviewer(risky_config(), Box::new(reviewer.clone()));
    // Marker in a nested argument string routes to review; approve passes.
    assert_eq!(
        inspector
            .inspect(&call(
                "execute_command",
                serde_json::json!({"nested": {"deep": ["run dangerous op"]}})
            ))
            .await
            .expect("inspect"),
        Verdict::Allow {
            authoritative: false
        }
    );
    assert_eq!(reviewer.reviewed(), 1);
    // Marker in the tool name routes to review.
    inspector
        .inspect(&call("dangerous_tool", serde_json::Value::Null))
        .await
        .expect("inspect");
    assert_eq!(reviewer.reviewed(), 2);
    // No marker: not reviewed at all.
    inspector
        .inspect(&call(
            "execute_command",
            serde_json::json!({"cmd": "cargo build"}),
        ))
        .await
        .expect("inspect");
    assert_eq!(reviewer.reviewed(), 2, "clean calls skip the reviewer");
}

#[tokio::test]
async fn reviewer_denial_blocks_with_feedback() {
    let reviewer = ReviewRecorder::with(vec![ReviewOutcome::Deny {
        reason: "policy violation: destructive sweep".to_owned(),
    }]);
    let mut inspector = SecurityInspector::with_reviewer(risky_config(), Box::new(reviewer));
    match inspector
        .inspect(&call(
            "execute_command",
            serde_json::json!({"cmd": "dangerous sweep"}),
        ))
        .await
        .expect("inspect")
    {
        Verdict::Deny { reason } => {
            assert!(reason.contains("security review denied"), "{reason}");
            assert!(reason.contains("policy violation"), "{reason}");
        }
        other => panic!("expected Deny, got {other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn hung_reviewer_times_out_and_fails_closed() {
    let mut inspector = SecurityInspector::with_reviewer(risky_config(), Box::new(HungReviewer));
    match inspector
        .inspect(&call(
            "execute_command",
            serde_json::json!({"cmd": "dangerous hang"}),
        ))
        .await
        .expect("inspect")
    {
        Verdict::Deny { reason } => {
            assert!(reason.contains("timed out"), "{reason}");
            assert!(reason.contains("failing closed"), "{reason}");
        }
        other => panic!("expected a fail-closed Deny, got {other:?}"),
    }
}

#[tokio::test]
async fn enabled_without_a_reviewer_denies_risky_calls() {
    let mut inspector = SecurityInspector::new(risky_config());
    match inspector
        .inspect(&call(
            "execute_command",
            serde_json::json!({"cmd": "dangerous op"}),
        ))
        .await
        .expect("inspect")
    {
        Verdict::Deny { reason } => {
            assert!(reason.contains("no reviewer configured"), "{reason}");
        }
        other => panic!("expected Deny, got {other:?}"),
    }
}

#[tokio::test]
async fn chain_consults_the_security_reviewer_when_enabled() {
    let mut config = InspectorConfig::default();
    config
        .permission
        .rules
        .push(Rule::tool(Effect::Allow, "*").expect("rule"));
    config.security = risky_config();
    let mut chain = InspectorChain::new(config);
    // Clean call passes straight through the allow rule.
    assert!(matches!(
        chain
            .evaluate(&call(
                "execute_command",
                serde_json::json!({"cmd": "cargo build"})
            ))
            .await
            .expect("evaluate"),
        ChainOutcome::Allow { .. }
    ));
    // Risky call hits the default fail-closed reviewer (none configured).
    match chain
        .evaluate(&call(
            "execute_command",
            serde_json::json!({"cmd": "dangerous op"}),
        ))
        .await
        .expect("evaluate")
    {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.contains("security review denied"), "{feedback}");
        }
        ChainOutcome::Allow { .. } => panic!("risky call must be reviewed"),
    }
}
