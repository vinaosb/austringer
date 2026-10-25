//! NL permission-hint tests (todo 8): the deterministic mini-grammar that
//! turns natural-language allow/deny/ask phrases into typed rules and
//! grants — including typed refusals for bypass attempts and garbage.

mod common;

use std::sync::Arc;

use austringer_core::autonomy::{HintParse, HintRefusal, ParsedHint, apply_hint, parse_hint};
use austringer_core::inspectors::chain::{ChainOutcome, InspectorChain, InspectorConfig};
use austringer_core::inspectors::permission::Effect;
use austringer_core::inspectors::store::{GrantScope, MemoryStore, PermissionStore};
use common::inspectorkit::call;
use serde_json::json;

fn refuse() -> Box<dyn austringer_core::inspectors::store::AskPolicy> {
    Box::new(austringer_core::inspectors::store::RefuseAsks)
}

#[tokio::test]
async fn always_allow_command_phrase_becomes_session_allow_rule() {
    let mut config = InspectorConfig::default();
    let store = MemoryStore::new();
    let outcome =
        apply_hint(&mut config.permission, &store, "always allow npm test").expect("apply");
    match outcome {
        HintParse::Applied { hints } => {
            assert_eq!(hints.len(), 1);
            match &hints[0] {
                ParsedHint::AllowRule(rule) => {
                    assert_eq!(rule.tool.pattern(), "execute_command");
                    let target = rule.target.as_ref().expect("command target");
                    assert_eq!(target.pattern.pattern(), "npm test*");
                }
                other => panic!("expected AllowRule, got {other:?}"),
            }
        }
        HintParse::Refused { reason } => panic!("refused: {reason:?}"),
    }
    // Session phrasing records no forever grant.
    assert_eq!(store.lookup("execute_command", Some("npm test")), None);
    // And the rule actually approves the command with no ask.
    let mut chain = InspectorChain::new(config).with_ask_policy(refuse());
    let outcome = chain
        .evaluate(&call(
            "execute_command",
            json!({"command": "npm test -- --nocapture"}),
        ))
        .await
        .expect("evaluate");
    assert!(matches!(outcome, ChainOutcome::Allow { .. }));
}

#[tokio::test]
async fn forever_phrase_also_records_a_persistent_grant() {
    let mut config = InspectorConfig::default();
    let store = MemoryStore::new();
    let outcome = apply_hint(
        &mut config.permission,
        &store,
        "always allow cargo build forever",
    )
    .expect("apply");
    match outcome {
        HintParse::Applied { hints } => {
            assert!(
                hints
                    .iter()
                    .any(|hint| matches!(hint, ParsedHint::AllowRule(_)))
            );
            assert!(
                hints
                    .iter()
                    .any(|hint| matches!(hint, ParsedHint::Grant(_)))
            );
        }
        HintParse::Refused { reason } => panic!("refused: {reason:?}"),
    }
    assert_eq!(
        store.lookup("execute_command", Some("cargo build")),
        Some(GrantScope::Forever)
    );
}

#[tokio::test]
async fn never_touch_phrase_becomes_deny_glob() {
    let mut config = InspectorConfig::default();
    let store = MemoryStore::new();
    let outcome = apply_hint(&mut config.permission, &store, "never touch .env").expect("apply");
    match outcome {
        HintParse::Applied { hints } => match &hints[0] {
            ParsedHint::DenyRule(rule) => {
                assert_eq!(rule.effect, Effect::Deny);
                let target = rule.target.as_ref().expect("path target");
                assert_eq!(target.pattern.pattern(), ".env*");
            }
            other => panic!("expected DenyRule, got {other:?}"),
        },
        HintParse::Refused { reason } => panic!("refused: {reason:?}"),
    }
    // The deny rule blocks even a read of the path, with model feedback.
    let mut chain = InspectorChain::new(config).with_ask_policy(refuse());
    let outcome = chain
        .evaluate(&call("view_window", json!({"path": ".env.local"})))
        .await
        .expect("evaluate");
    match outcome {
        ChainOutcome::Block { feedback } => assert!(feedback.contains("deny rule")),
        other => panic!("expected Block, got {other:?}"),
    }
}

#[tokio::test]
async fn ask_before_deleting_phrase_covers_the_delete_family() {
    let mut config = InspectorConfig::default();
    let store = MemoryStore::new();
    let outcome = apply_hint(&mut config.permission, &store, "ask before deleting").expect("apply");
    match outcome {
        HintParse::Applied { hints } => {
            assert!(hints.len() >= 2, "rm + del at minimum");
            for hint in &hints {
                match hint {
                    ParsedHint::AskRule(rule) => {
                        assert_eq!(rule.effect, Effect::Ask);
                        assert_eq!(rule.tool.pattern(), "execute_command");
                    }
                    other => panic!("expected AskRule, got {other:?}"),
                }
            }
        }
        HintParse::Refused { reason } => panic!("refused: {reason:?}"),
    }
    // rm now routes to an ask; with the headless policy it is refused.
    let mut chain = InspectorChain::new(config).with_ask_policy(refuse());
    let outcome = chain
        .evaluate(&call("execute_command", json!({"command": "rm old.txt"})))
        .await
        .expect("evaluate");
    match outcome {
        ChainOutcome::Block { feedback } => assert!(feedback.contains("ask was refused")),
        other => panic!("expected refused Block, got {other:?}"),
    }
}

#[test]
fn always_allow_everything_is_a_typed_refusal_not_a_global_allow() {
    let mut config = InspectorConfig::default();
    let store = MemoryStore::new();
    let outcome =
        apply_hint(&mut config.permission, &store, "always allow everything").expect("apply");
    match outcome {
        HintParse::Refused { reason } => {
            assert!(matches!(reason, HintRefusal::BypassAttempt { .. }))
        }
        HintParse::Applied { .. } => panic!("global allow must not be applied"),
    }
    assert!(config.permission.rules.is_empty(), "no rule leaked");
    assert!(store.lookup("*", None).is_none(), "no grant leaked");
}

#[test]
fn bypass_and_yolo_phrases_are_typed_refusals() {
    for phrase in ["bypass permissions", "yolo mode", "allow all tools"] {
        match parse_hint(phrase) {
            HintParse::Refused {
                reason: HintRefusal::BypassAttempt { .. },
            } => {}
            other => panic!("phrase {phrase:?} should refuse, got {other:?}"),
        }
    }
}

#[test]
fn garbage_phrases_get_typed_feedback_never_silently_ignored() {
    match parse_hint("make it fast please") {
        HintParse::Refused {
            reason: HintRefusal::Unrecognized { phrase },
        } => assert_eq!(phrase, "make it fast please"),
        other => panic!("expected Unrecognized, got {other:?}"),
    }
    // An empty-ish phrase is also unrecognized, never an implicit allow.
    assert!(matches!(
        parse_hint("   "),
        HintParse::Refused {
            reason: HintRefusal::Unrecognized { .. }
        }
    ));
}

#[tokio::test]
async fn applied_hints_reach_the_store_via_shared_handle() {
    // The store behind an Arc<dyn> is the production wiring: hints applied
    // to it are visible to every chain sharing the store.
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let mut config = InspectorConfig::default();
    apply_hint(
        &mut config.permission,
        store.as_ref(),
        "always allow npm test forever",
    )
    .expect("apply");
    assert_eq!(
        store.lookup("execute_command", Some("npm test")),
        Some(GrantScope::Forever)
    );
}
