//! Chain precedence tests (todo 7): an authoritative allow suppresses
//! policy asks where the policy says so, deny beats allow, and the
//! repetition intervention is never suppressible by an allow rule.

mod common;

use austringer_core::inspectors::chain::{ChainOutcome, InspectorChain, InspectorConfig};
use austringer_core::inspectors::permission::{Effect, Rule};
use common::inspectorkit::{allow_config, call};

#[tokio::test]
async fn authoritative_allow_suppresses_policy_asks_where_policy_says_so() {
    let asks = common::inspectorkit::CaptureAsks::default();
    let mut config = InspectorConfig::default();
    config
        .permission
        .rules
        .push(Rule::path(Effect::Allow, "view_window", "**").expect("rule"));
    let mut chain = InspectorChain::new(config).with_ask_policy(Box::new(asks.clone()));
    // The explicit allow covers the secret file: no ask is generated.
    let outcome = chain
        .evaluate(&call(
            "view_window",
            serde_json::json!({"path": "prod/.env"}),
        ))
        .await
        .expect("evaluate");
    assert!(
        matches!(outcome, ChainOutcome::Allow { notice: None }),
        "explicit allow suppresses the secret guard's ask: {outcome:?}"
    );
    assert!(asks.asked().is_empty());
}

#[tokio::test]
async fn deny_beats_allow_in_the_chain() {
    let mut config = InspectorConfig::default();
    config
        .permission
        .rules
        .push(Rule::path(Effect::Allow, "view_window", "**").expect("rule"));
    config
        .permission
        .rules
        .push(Rule::path(Effect::Deny, "view_window", "**/.env").expect("rule"));
    let mut chain = InspectorChain::new(config);
    match chain
        .evaluate(&call(
            "view_window",
            serde_json::json!({"path": "prod/.env"}),
        ))
        .await
        .expect("evaluate")
    {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.contains("deny rule"), "{feedback}");
            // The actual secret path is masked; the rule pattern itself
            // ('**/.env') is user configuration and may appear.
            assert!(!feedback.contains("prod/.env"), "{feedback}");
            assert!(feedback.contains("[REDACTED"), "{feedback}");
        }
        ChainOutcome::Allow { .. } => panic!("deny must beat allow"),
    }
}

#[tokio::test]
async fn repetition_intervention_is_not_suppressed_by_an_allow_rule() {
    let asks = common::inspectorkit::CaptureAsks::default();
    let mut chain = InspectorChain::new(allow_config("*")).with_ask_policy(Box::new(asks));
    let args = serde_json::json!({"path": "same.txt"});
    for index in 1..=2 {
        let outcome = chain
            .evaluate(&call("read_file", args.clone()))
            .await
            .expect("evaluate");
        assert!(
            matches!(outcome, ChainOutcome::Allow { .. }),
            "call {index} passes"
        );
    }
    match chain
        .evaluate(&call("read_file", args))
        .await
        .expect("evaluate")
    {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.contains("repetition intervention"), "{feedback}");
            assert!(feedback.contains("3 times"), "{feedback}");
            assert!(feedback.contains("vary"), "{feedback}");
        }
        ChainOutcome::Allow { .. } => panic!("third identical call must trip"),
    }
}
