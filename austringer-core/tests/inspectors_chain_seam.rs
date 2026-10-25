//! Inspector-chain seam tests (todo 7): the chain wired into the todo-5
//! `ToolApproval` stage of a real `AgentLoop` — allow short-circuits to
//! execution, deny returns structured model-feedback, execution is
//! structurally impossible before the chain passes, and the fallback ask
//! fires when no inspector has an opinion. Precedence tests live in
//! inspectors_precedence.rs; secret-file flows in inspectors_secret_files.rs.

mod common;

use austringer_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome, StageSet};
use austringer_core::inspectors::chain::{ChainOutcome, InspectorChain, InspectorConfig};
use austringer_core::inspectors::permission::{Effect, Rule};
use austringer_core::inspectors::store::{AskRequest, AskResolution};
use common::TempSession;
use common::inspectorkit::{allow_config, call, calls_turn, tool_result_texts};
use common::loopkit::{ScriptedInference, call_turn, text_turn};

#[tokio::test]
async fn allow_rule_short_circuits_the_chain_and_executes() {
    let dir = TempSession::new("insp-allow");
    let asks = common::inspectorkit::CaptureAsks::default();
    let chain = InspectorChain::new(allow_config("view_*")).with_ask_policy(Box::new(asks.clone()));
    let executor = common::inspectorkit::RecordingExecutor::default();
    let stages = StageSet {
        approval: Box::new(chain),
        execution: Box::new(executor.clone()),
        ..StageSet::default()
    };
    let inference = ScriptedInference::new(vec![
        call_turn(
            "c1",
            "view_window",
            serde_json::json!({"path": "src/main.rs"}),
        ),
        text_turn("done"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("read the file");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    // Transcript: PermissionInspector Allow -> no Ask generated anywhere
    // (the capture queue would have refused; nothing was even asked).
    assert!(
        asks.asked().is_empty(),
        "allow must short-circuit ask generation"
    );
    let executed = executor.calls();
    assert_eq!(executed.len(), 1);
    assert_eq!(executed[0].tool, "view_window");
    assert_eq!(executed[0].arguments["path"], "src/main.rs");
    assert_eq!(
        tool_result_texts(session.events()),
        vec!["c1: executed-ok".to_owned()]
    );
}

#[tokio::test]
async fn deny_blocks_execution_and_returns_model_feedback() {
    let dir = TempSession::new("insp-deny");
    let mut config = InspectorConfig::default();
    config
        .permission
        .rules
        .push(Rule::path(Effect::Deny, "read_file", "**/*.pem").expect("rule"));
    let executor = common::inspectorkit::RecordingExecutor::default();
    let stages = StageSet {
        approval: Box::new(InspectorChain::new(config)),
        execution: Box::new(executor.clone()),
        ..StageSet::default()
    };
    let inference = ScriptedInference::new(vec![
        call_turn(
            "c1",
            "read_file",
            serde_json::json!({"path": "keys/prod.pem"}),
        ),
        text_turn("okay, I will not read that"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("read the key");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    // Structurally impossible to execute before the chain passes: the
    // executor never saw the call.
    assert!(executor.calls().is_empty(), "denied call must not execute");
    // The model-visible deny feedback is the typed structured reason,
    // redacted, wrapped by the approval stage.
    assert_eq!(
        tool_result_texts(session.events()),
        vec![
            "c1: tool call denied by approval stage: permission denied: tool 'read_file' \
             target '[REDACTED:secret-path]' matched deny rule '**/*.pem'"
                .to_owned()
        ]
    );
}

#[tokio::test]
async fn mixed_batch_executes_only_the_allowed_calls() {
    let dir = TempSession::new("insp-mixed");
    let mut config = InspectorConfig::default();
    config
        .permission
        .rules
        .push(Rule::tool(Effect::Allow, "view_*").expect("rule"));
    config
        .permission
        .rules
        .push(Rule::tool(Effect::Deny, "read_file").expect("rule"));
    let executor = common::inspectorkit::RecordingExecutor::default();
    let stages = StageSet {
        approval: Box::new(InspectorChain::new(config)),
        execution: Box::new(executor.clone()),
        ..StageSet::default()
    };
    let inference = ScriptedInference::new(vec![
        calls_turn(vec![
            ("c1", "view_window", serde_json::json!({"path": "src/a.rs"})),
            ("c2", "read_file", serde_json::json!({"path": "secret.txt"})),
        ]),
        text_turn("done"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("read both");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let executed = executor.calls();
    assert_eq!(executed.len(), 1, "only the allowed call executes");
    assert_eq!(executed[0].call_id, "c1");
    let results = tool_result_texts(session.events());
    assert!(results.contains(&"c1: executed-ok".to_owned()));
    assert!(
        results
            .iter()
            .any(|text| text.starts_with("c2: tool call denied by approval stage"))
    );
}

#[tokio::test]
async fn chain_falls_back_to_an_ask_when_no_inspector_has_an_opinion() {
    let asks = common::inspectorkit::CaptureAsks::with(vec![AskResolution::Refuse]);
    let mut chain =
        InspectorChain::new(InspectorConfig::default()).with_ask_policy(Box::new(asks.clone()));
    match chain
        .evaluate(&call(
            "write_file",
            serde_json::json!({"path": "notes.txt"}),
        ))
        .await
        .expect("evaluate")
    {
        ChainOutcome::Block { feedback } => {
            assert!(
                feedback.contains("no matching permission rule"),
                "{feedback}"
            );
            assert!(feedback.contains("notes.txt"), "{feedback}");
        }
        ChainOutcome::Allow { .. } => panic!("no-rule call must ask, not silently allow"),
    }
    let asked: Vec<AskRequest> = asks.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].tool, "write_file");
    // A secret-looking unmatched target stays masked even in the fallback.
    let mut chain2 = InspectorChain::new(InspectorConfig::default());
    match chain2
        .evaluate(&call(
            "write_file",
            serde_json::json!({"path": "prod/.env"}),
        ))
        .await
        .expect("evaluate")
    {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.contains("[REDACTED:secret-path]"), "{feedback}");
            assert!(!feedback.contains("prod/.env"), "{feedback}");
        }
        ChainOutcome::Allow { .. } => panic!("fallback ask must fire"),
    }
}
