//! Secret-file guard tests (todo 7): reads of high-risk paths default to
//! ASK with a redacted reason; the headless default refuses; a `y` grant
//! executes once with the REAL path while the transcript gets a REDACTED
//! placeholder; an `a` grant stores and auto-allows subsequent matching
//! calls.

mod common;

use std::sync::Arc;

use common::TempSession;
use common::inspectorkit::{RecordingExecutor, system_texts, tool_result_texts};
use common::loopkit::{ScriptedInference, call_turn, text_turn};
use harness_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome, StageSet};
use harness_core::inspectors::chain::{ChainOutcome, InspectorChain, InspectorConfig};
use harness_core::inspectors::store::{AskResolution, GrantScope, MemoryStore, PermissionStore};

#[tokio::test]
async fn secret_read_in_default_config_produces_an_ask_verdict() {
    let asks = common::inspectorkit::CaptureAsks::with(vec![AskResolution::Refuse]);
    let mut chain =
        InspectorChain::new(InspectorConfig::default()).with_ask_policy(Box::new(asks.clone()));

    // The verdict is an ASK, observable through the routed request: the
    // approving human sees the real target; the reason stays redacted.
    let outcome = chain
        .evaluate(&common::inspectorkit::call(
            "view_window",
            serde_json::json!({"path": ".env"}),
        ))
        .await
        .expect("evaluate");
    let asked = asks.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].tool, "view_window");
    assert_eq!(asked[0].target.as_deref(), Some(".env"));
    assert!(
        asked[0].reason.contains("secret-file read"),
        "{}",
        asked[0].reason
    );
    assert!(
        !asked[0].reason.contains(".env"),
        "reason must stay redacted"
    );

    // Headless default (RefuseAsks): the block feedback carries the
    // redacted reason, never the path.
    match outcome {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.contains("approval required"), "{feedback}");
            assert!(feedback.contains("secret-file read"), "{feedback}");
            assert!(
                !feedback.contains(".env"),
                "feedback must not leak the path"
            );
        }
        ChainOutcome::Allow { .. } => panic!("expected Block"),
    }
}

#[tokio::test]
async fn secret_read_headless_default_refuses_without_a_policy() {
    let mut chain = InspectorChain::new(InspectorConfig::default());
    match chain
        .evaluate(&common::inspectorkit::call(
            "read_file",
            serde_json::json!({"path": "config/id_rsa_test"}),
        ))
        .await
        .expect("evaluate")
    {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.contains("refused"), "{feedback}");
            assert!(!feedback.contains("id_rsa"), "{feedback}");
        }
        ChainOutcome::Allow { .. } => panic!("headless default must refuse asks"),
    }
}

#[tokio::test]
async fn secret_read_approved_once_executes_with_real_path_and_redacts_transcript() {
    let dir = TempSession::new("insp-secret-y");
    let asks = common::inspectorkit::CaptureAsks::with(vec![AskResolution::GrantOnce]);
    let executor = RecordingExecutor::default();
    let stages = StageSet {
        approval: Box::new(
            InspectorChain::new(InspectorConfig::default()).with_ask_policy(Box::new(asks.clone())),
        ),
        execution: Box::new(executor.clone()),
        ..StageSet::default()
    };
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "view_window", serde_json::json!({"path": ".env"})),
        call_turn("c2", "view_window", serde_json::json!({"path": ".env"})),
        text_turn("got it"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("check the environment");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    // The tool received the real path — exactly once: `y` grants one call,
    // and the second identical call asks again with an empty queue, which
    // refuses.
    let executed = executor.calls();
    assert_eq!(executed.len(), 1);
    assert_eq!(executed[0].arguments["path"], ".env");
    assert_eq!(executed[0].call_id, "c1");
    assert_eq!(asks.asked().len(), 2, "the second call asks again");
    assert!(
        tool_result_texts(session.events())
            .iter()
            .any(|text| text.starts_with("c2: tool call denied by approval stage"))
    );
    // Everything the approval layer put into the transcript is a redacted
    // placeholder.
    let notices = system_texts(session.events());
    assert!(
        notices
            .iter()
            .any(|text| text.contains("secret-file read approved")
                && text.contains("[REDACTED]")
                && !text.contains(".env")),
        "redacted approval notice: {notices:?}"
    );
}

#[tokio::test]
async fn secret_read_a_grant_auto_allows_subsequent_matching_calls() {
    let store = Arc::new(MemoryStore::new());
    let asks = common::inspectorkit::CaptureAsks::with(vec![AskResolution::GrantAlways(
        GrantScope::Session,
    )]);
    let mut chain = InspectorChain::with_store(InspectorConfig::default(), store.clone())
        .with_ask_policy(Box::new(asks.clone()));

    let first = chain
        .evaluate(&common::inspectorkit::call(
            "view_window",
            serde_json::json!({"path": ".env"}),
        ))
        .await
        .expect("evaluate");
    assert!(
        matches!(first, ChainOutcome::Allow { notice: Some(_) }),
        "granted ask allows with a redaction notice: {first:?}"
    );
    let second = chain
        .evaluate(&common::inspectorkit::call(
            "view_window",
            serde_json::json!({"path": ".env"}),
        ))
        .await
        .expect("evaluate");
    assert!(matches!(second, ChainOutcome::Allow { .. }), "{second:?}");
    // Exactly ONE ask ever happened: the stored grant answered the rest.
    assert_eq!(asks.asked().len(), 1);
    assert_eq!(
        store.lookup("view_window", Some(".env")),
        Some(GrantScope::Session)
    );
    // Session scope: cleared at session end.
    store.clear_session().expect("clear");
    assert_eq!(store.lookup("view_window", Some(".env")), None);
}
