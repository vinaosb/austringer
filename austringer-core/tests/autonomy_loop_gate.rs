//! Loop-integration tests (todo 8): the /mode slash command driving the
//! shared matrix, plan-mode freezing write tools at the dispatch gate
//! while the serialized schema stays byte-identical, artifact review
//! sync onto the plan gate, and slash delegation for non-/mode commands.

mod common;

use std::sync::Arc;

use austringer_core::agent_loop::{AgentLoop, LoopConfig, PlanGate, RunOutcome, StageSet};
use austringer_core::autonomy::{
    AutonomyHandle, AutonomyMatrix, AutonomyMode, AutonomyParts, ModeCommand, autonomy_chain,
};
use austringer_core::event::{ContentBlock, Event};
use austringer_core::inspectors::chain::InspectorConfig;
use austringer_core::inspectors::store::{MemoryStore, PermissionStore, RefuseAsks};
use austringer_core::tools::{IoCap, ToolRegistry};
use common::autonomykit::{ScriptedIntent, ScriptedRisk};
use common::inspectorkit::{RecordingExecutor, system_texts};
use common::loopkit::{ScriptedInference, call_turn, text_turn};
use common::stages_kit::stages;
use serde_json::json;

fn stage_set(
    handle: &AutonomyHandle,
    store: Arc<dyn PermissionStore>,
    plan_gate: Option<PlanGate>,
    executor: RecordingExecutor,
) -> StageSet {
    let chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(RefuseAsks),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    stages(|s| {
        s.slash_commands = Box::new(ModeCommand::new(handle.shared(), plan_gate));
        s.approval = Box::new(chain);
        s.execution = Box::new(executor);
    })
}

fn text_of(events: &[austringer_core::event::RecordedEvent], call_id: &str) -> String {
    events
        .iter()
        .filter_map(|recorded| match &recorded.event {
            Event::ToolResult { call_id, blocks } => Some((
                call_id.clone(),
                blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            )),
            _ => None,
        })
        .find(|(id, _)| id == call_id)
        .map(|(_, text)| text)
        .unwrap_or_default()
}

#[tokio::test(start_paused = true)]
async fn plan_mode_freezes_write_tools_at_the_dispatch_gate() {
    // QA happy path 2: /mode plan + a read+write turn. The write tool is
    // denied with plan-mode feedback, the read executes, and the base
    // schema snapshot stays byte-identical across the mode switch.
    let session = common::TempSession::new("plan-freeze");
    let registry_dir = common::TempSession::new("plan-freeze-registry");
    let registry = ToolRegistry::open(registry_dir.path(), IoCap::default()).expect("registry");
    let pinned = registry.pin_current();
    let schema_before = registry.render_base_schema(&pinned);

    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::SmartApprove));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let executor = RecordingExecutor::default();
    let mut session_loop = AgentLoop::open(
        session.path(),
        LoopConfig::default(),
        stage_set(&handle, store, None, executor.clone()),
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

    session_loop.submit("/mode plan");
    session_loop.submit("make the edits");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);

    let executed = executor.calls();
    assert_eq!(executed.len(), 1, "only the read may execute");
    assert_eq!(executed[0].tool, "view_window");

    let feedback = text_of(session_loop.events(), "c1");
    assert!(
        feedback.to_lowercase().contains("plan mode"),
        "feedback: {feedback}"
    );

    assert_eq!(
        registry.render_base_schema(&pinned),
        schema_before,
        "schema must stay byte-identical across the mode switch"
    );
}

#[tokio::test(start_paused = true)]
async fn leaving_plan_mode_restores_dispatch() {
    // Outside plan mode the same edit routes to the SmartApprove
    // classifier; with no scripted verdict the stage fails safe to an ask,
    // which the headless policy refuses — a denial, not a freeze.
    let session = common::TempSession::new("plan-unfreeze");
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::SmartApprove));
    handle.write(|matrix| matrix.enter_plan());
    let mut session_loop = AgentLoop::open(
        session.path(),
        LoopConfig::default(),
        stage_set(
            &handle,
            Arc::new(MemoryStore::new()),
            None,
            RecordingExecutor::default(),
        ),
        Box::new(ScriptedInference::new(vec![
            call_turn(
                "c1",
                "edit_lines",
                json!({"path": "src/main.rs", "edits": []}),
            ),
            text_turn("done"),
        ])),
    )
    .expect("open");
    session_loop.submit("edit");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert!(
        text_of(session_loop.events(), "c1")
            .to_lowercase()
            .contains("plan mode")
    );

    session_loop.submit("/mode smartapprove");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    session_loop.set_inference(Box::new(ScriptedInference::new(vec![
        call_turn(
            "c2",
            "edit_lines",
            json!({"path": "src/main.rs", "edits": []}),
        ),
        text_turn("done"),
    ])));
    session_loop.submit("edit again");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    let feedback = text_of(session_loop.events(), "c2").to_lowercase();
    assert!(
        !feedback.contains("plan mode"),
        "no freeze outside plan mode: {feedback}"
    );
}

#[tokio::test(start_paused = true)]
async fn mode_command_sets_modes_and_reports_deterministically() {
    let session = common::TempSession::new("mode-cmd");
    let handle = AutonomyHandle::new(AutonomyMatrix::default());
    let mut session_loop = AgentLoop::open(
        session.path(),
        LoopConfig::default(),
        stage_set(
            &handle,
            Arc::new(MemoryStore::new()),
            None,
            RecordingExecutor::default(),
        ),
        Box::new(ScriptedInference::new(vec![text_turn("unused")])),
    )
    .expect("open");
    session_loop.submit("/mode smartapprove");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert_eq!(
        handle.read(|matrix| matrix.mode()),
        AutonomyMode::SmartApprove
    );
    assert!(
        system_texts(session_loop.events())
            .iter()
            .any(|text| text.to_lowercase().contains("smartapprove")),
    );

    session_loop.submit("/mode bogus");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert_eq!(
        handle.read(|matrix| matrix.mode()),
        AutonomyMode::SmartApprove
    );
    assert!(
        system_texts(session_loop.events())
            .iter()
            .any(|text| text.to_lowercase().contains("unknown")),
    );
}

#[tokio::test(start_paused = true)]
async fn mode_command_delegates_non_mode_input() {
    // Other slash commands stay unknown (preserves the builtin posture);
    // plain text reaches Steer as a user event.
    let session = common::TempSession::new("mode-delegate");
    let handle = AutonomyHandle::new(AutonomyMatrix::default());
    let mut session_loop = AgentLoop::open(
        session.path(),
        LoopConfig::default(),
        stage_set(
            &handle,
            Arc::new(MemoryStore::new()),
            None,
            RecordingExecutor::default(),
        ),
        Box::new(ScriptedInference::new(vec![text_turn("ok")])),
    )
    .expect("open");
    session_loop.submit("/help");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert!(
        system_texts(session_loop.events())
            .iter()
            .any(|text| text.contains("unknown command")),
        "non-/mode slash stays unknown"
    );

    session_loop.submit("plain text");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert!(
        session_loop
            .events()
            .iter()
            .any(|recorded| matches!(&recorded.event, Event::User { .. })),
        "plain text is steered, not swallowed"
    );
}

#[tokio::test(start_paused = true)]
async fn review_policy_syncs_onto_the_plan_gate() {
    // Artifact-gated approval: Request-Review hard-blocks task completion
    // until an artifact-review verdict clears the gate; Always-Proceed
    // leaves it open.
    let session = common::TempSession::new("review-sync");
    let handle = AutonomyHandle::new(AutonomyMatrix::default());
    let gate = PlanGate::new();
    let mut session_loop = AgentLoop::open(
        session.path(),
        LoopConfig::default(),
        stage_set(
            &handle,
            Arc::new(MemoryStore::new()),
            Some(gate.clone()),
            RecordingExecutor::default(),
        ),
        Box::new(ScriptedInference::new(vec![text_turn("unused")])),
    )
    .expect("open");
    // Default (Chat) is RequestReview: blocked.
    session_loop.submit("/mode");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert!(gate.is_hard_blocked());
    // Auto defaults to AlwaysProceed: open.
    session_loop.submit("/mode auto");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert!(!gate.is_hard_blocked());
    // Back to a Request-Review mode: blocked again.
    session_loop.submit("/mode approve");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert!(gate.is_hard_blocked());
    // The artifact-review verdict itself is the clearing action.
    gate.clear();
    assert!(!gate.is_hard_blocked());
}

#[tokio::test(start_paused = true)]
async fn task_completion_defers_while_review_is_requested() {
    // Full wiring: a hard-blocked review gate defers task_completed with
    // the typed plan-gate block; clearing the gate (the artifact verdict)
    // lets a retry complete.
    let session = common::TempSession::new("review-defer");
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::SmartApprove));
    let gate = PlanGate::new();
    gate.hard_block();
    let mut session_loop = AgentLoop::open(
        session.path(),
        LoopConfig::default(),
        stage_set(
            &handle,
            Arc::new(MemoryStore::new()),
            Some(gate.clone()),
            RecordingExecutor::default(),
        ),
        Box::new(ScriptedInference::new(vec![
            call_turn("c1", "task_completed", json!({})),
            call_turn("c2", "task_completed", json!({})),
            text_turn("wrap up"),
        ])),
    )
    .expect("open");
    // The loop enforces the SAME handle /mode syncs: replace the loop-local
    // default with the shared, already-blocked gate.
    session_loop.set_plan_gate(gate.clone());
    session_loop.submit("finish");
    assert_eq!(session_loop.run().await, RunOutcome::TurnComplete);
    assert!(
        system_texts(session_loop.events())
            .iter()
            .any(|text| text.contains("plan.md gate")),
        "the plan-gate block notice must surface"
    );
    assert!(
        session_loop.last_completion_block().is_some(),
        "task-level completion deferred with a typed block"
    );
    // The artifact-review verdict arrives; the model's retry completes.
    gate.clear();
    session_loop.set_inference(Box::new(ScriptedInference::new(vec![call_turn(
        "c3",
        "task_completed",
        json!({"after_review": true}),
    )])));
    session_loop.submit("retry completion");
    assert_eq!(session_loop.run().await, RunOutcome::TaskCompleted);
}
