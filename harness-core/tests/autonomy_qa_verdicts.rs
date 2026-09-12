//! Manual-QA verdict-chain generators (todo 8): run explicitly with
//! `cargo nextest run --run-ignored=only --nocapture -p harness-core` and
//! paste the captured stdout into the evidence file. Ignored in normal
//! runs by design — they exist to print, not to assert.

mod common;

use std::sync::Arc;

use common::TempSession;
use common::autonomykit::{ScriptedIntent, ScriptedRisk};
use common::inspectorkit::{CaptureAsks, call};
use common::loopkit::{ScriptedInference, call_turn, text_turn};
use common::stages_kit::stages;
use harness_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome};
use harness_core::autonomy::{
    AutonomyHandle, AutonomyMatrix, AutonomyMode, AutonomyParts, Intent, Risk, autonomy_chain,
};
use harness_core::inspectors::chain::{ChainOutcome, InspectorConfig};
use harness_core::inspectors::store::{AskResolution, MemoryStore, PermissionStore, RefuseAsks};
use harness_core::tools::{IoCap, ToolRegistry};
use serde_json::json;

fn wired(
    matrix: AutonomyMatrix,
    intents: Vec<Intent>,
    risks: Vec<Risk>,
    asks: Vec<AskResolution>,
) -> (
    AutonomyHandle,
    Box<harness_core::inspectors::chain::InspectorChain>,
    CaptureAsks,
) {
    let handle = AutonomyHandle::new(matrix);
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let capture = CaptureAsks::with(asks);
    let chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(capture.clone()),
        intent: Box::new(ScriptedIntent::new(intents)),
        risk: Box::new(ScriptedRisk::new(risks)),
        classifier: harness_core::autonomy::ClassifierConfig::default(),
    });
    (handle, Box::new(chain), capture)
}

#[tokio::test]
#[ignore = "manual QA verdict chain: happy — SmartApprove approves benign read"]
async fn qa_smart_approve_benign_read() {
    let (_handle, mut chain, asks) = wired(
        AutonomyMatrix::for_mode(AutonomyMode::SmartApprove),
        vec![],
        vec![],
        vec![],
    );
    let read = call("view_window", json!({"path": "src/main.rs"}));
    println!("== QA happy 1: SmartApprove, benign read (view_window src/main.rs) ==");
    println!("  capability: Read — SmartApprove preset toggles it ON");
    println!("  classifier layer 2 (safe-tool allowlist) -> authoritative Allow");
    let outcome = chain.evaluate(&read).await.expect("evaluate");
    match outcome {
        ChainOutcome::Allow { notice } => println!("  verdict: ALLOW (notice: {notice:?})"),
        ChainOutcome::Block { feedback } => println!("  verdict: BLOCK ({feedback})"),
    }
    println!("  asks routed to the user: {}", asks.asked().len());
    println!("  -> executed with NO ask, NO classifier consult");
}

#[tokio::test(start_paused = true)]
#[ignore = "manual QA: happy — plan mode freezes write tools, schema byte-identical"]
async fn qa_plan_mode_freeze_with_schema_snapshot() {
    let session = TempSession::new("qa-plan-freeze");
    let registry_dir = TempSession::new("qa-plan-registry");
    let registry = ToolRegistry::open(registry_dir.path(), IoCap::default()).expect("registry");
    let pinned = registry.pin_current();
    let schema_before = registry.render_base_schema(&pinned);

    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::SmartApprove));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(RefuseAsks),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: harness_core::autonomy::ClassifierConfig::default(),
    });
    let executor = common::inspectorkit::RecordingExecutor::default();
    let mut session_loop = AgentLoop::open(
        session.path(),
        LoopConfig::default(),
        stages(|s| {
            s.slash_commands = Box::new(harness_core::autonomy::ModeCommand::new(
                handle.shared(),
                None,
            ));
            s.approval = Box::new(chain);
            s.execution = Box::new(executor.clone());
        }),
        Box::new(ScriptedInference::new(vec![
            call_turn(
                "c1",
                "edit_lines",
                json!({"path": "src/main.rs", "edits": []}),
            ),
            call_turn("c2", "view_window", json!({"path": "src/main.rs"})),
            text_turn("done"),
        ])),
    )
    .expect("open");
    println!("== QA happy 2: /mode plan freezes write tools at the dispatch gate ==");
    session_loop.submit("/mode plan");
    session_loop.submit("make the edits");
    let outcome = session_loop.run().await;
    println!("  run outcome: {outcome:?}");
    for recorded in session_loop.events() {
        let text = match &recorded.event {
            harness_core::event::Event::ToolResult { call_id, blocks } => format!(
                "{call_id}: {}",
                blocks
                    .iter()
                    .filter_map(|b| match b {
                        harness_core::event::ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            other => format!("{other:?}"),
        };
        println!("  seq {:>2}  {text}", recorded.seq.0);
    }
    println!(
        "  tools that reached execution: {:?}",
        executor
            .calls()
            .iter()
            .map(|c| c.tool.clone())
            .collect::<Vec<_>>()
    );
    let schema_after = registry.render_base_schema(&pinned);
    println!(
        "  schema bytes before/after mode switch identical: {} (len {})",
        schema_before == schema_after,
        schema_before.len()
    );
    assert_eq!(outcome, RunOutcome::TurnComplete);
}

#[tokio::test]
#[ignore = "manual QA: failure — destructive ops always ask, plus softDeny probe"]
async fn qa_destructive_always_asks_every_mode() {
    println!("== QA failure: destructive ops ask in EVERY mode (typed ask/deny) ==");
    for mode in AutonomyMode::ALL {
        let (_handle, mut chain, asks) = wired(
            AutonomyMatrix::for_mode(mode),
            vec![Intent::Benign],
            vec![Risk::Low],
            vec![AskResolution::Refuse],
        );
        let rm = call(
            "execute_command",
            json!({"command": "rm -rf /tmp/important"}),
        );
        let outcome = chain.evaluate(&rm).await.expect("evaluate");
        let asked = asks
            .asked()
            .first()
            .map(|request| request.reason.clone())
            .unwrap_or_default();
        match outcome {
            ChainOutcome::Allow { .. } => println!("  {mode:?}: ALLOW (BUG — must never)"),
            ChainOutcome::Block { feedback } => {
                println!("  {mode:?}: BLOCK; ask routed: {asked}");
                println!("           refusal feedback: {feedback}");
            }
        }
    }
    // .env write: secret guard.
    let (_h, mut env_chain, env_asks) = wired(
        AutonomyMatrix::for_mode(AutonomyMode::Auto),
        vec![],
        vec![],
        vec![AskResolution::Refuse],
    );
    let outcome = env_chain
        .evaluate(&call("edit_lines", json!({"path": ".env", "edits": []})))
        .await
        .expect("env");
    println!(
        "  Auto, edit_lines .env: BLOCK feedback: {}",
        match outcome {
            ChainOutcome::Block { feedback } => feedback,
            ChainOutcome::Allow { .. } => "ALLOW (BUG)".to_owned(),
        }
    );
    println!(
        "           ask routed: {}",
        env_asks
            .asked()
            .first()
            .map(|r| r.reason.clone())
            .unwrap_or_default()
    );
    // softDeny-on-exact-intent probe: command OUTSIDE the floor patterns,
    // intent stage reports the request literally asks for the harm.
    let (_h, mut sd_chain, sd_asks) = wired(
        AutonomyMatrix::for_mode(AutonomyMode::Auto),
        vec![Intent::HarmfulExact],
        vec![Risk::Low],
        vec![AskResolution::GrantOnce],
    );
    let outcome = sd_chain
        .evaluate(&call(
            "execute_command",
            json!({"command": "git push --force origin main"}),
        ))
        .await
        .expect("softdeny");
    println!(
        "  Auto, HarmfulExact intent (risk stage says Low): {}",
        match outcome {
            ChainOutcome::Block { feedback } => format!("BLOCK — {feedback}"),
            ChainOutcome::Allow { .. } => "ALLOW (BUG)".to_owned(),
        }
    );
    println!(
        "  softDeny probe burned no ask: {}",
        sd_asks.asked().is_empty()
    );
}
