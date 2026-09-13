//! Stop hooks (todo 28): block-until-criterion via the prompt-evaluator
//! seam, bounded by the hook-system loop limit (typed exhaustion), the
//! except-Stop-semantics failure rule (loud fail-open), and budget
//! persistence across runs.

mod common;

use common::TempSession;
use common::hookkit::payloads;
use common::loopkit::{ScriptedInference, text_turn};
use harness_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome, StageSet};
use harness_core::event::Event;
use harness_core::hooks::{HookEvent, HookKind, HookManager, HookSpec, PolicyVerdict, SharedHooks};

fn prompt_hook(name: &str, policy: &str) -> HookSpec {
    HookSpec::new(
        name,
        &[HookEvent::Stop],
        HookKind::Prompt {
            policy: policy.to_owned(),
        },
    )
}

fn stop_stages(manager: HookManager, block_cap: u32) -> (SharedHooks, StageSet, LoopConfig) {
    let shared = SharedHooks::new(manager);
    let stages = common::stages_kit::stages(|s| s.stop_hook = Box::new(shared.stop_stage()));
    let config = LoopConfig {
        stop_hook_block_cap: block_cap,
        ..LoopConfig::default()
    };
    (shared, stages, config)
}

fn system_texts(session: &AgentLoop) -> Vec<String> {
    session
        .events()
        .iter()
        .filter_map(|r| match &r.event {
            Event::System { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn stop_hook_blocks_until_criterion_bounded_by_loop_limit() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.evaluator.push(PolicyVerdict::ContinueWorking {
        reason: "tests still failing".to_owned(),
    });
    seams.evaluator.push(PolicyVerdict::ContinueWorking {
        reason: "docs not updated".to_owned(),
    });
    seams.evaluator.push(PolicyVerdict::ContinueWorking {
        reason: "one more check".to_owned(),
    });
    let mut manager = HookManager::new(
        vec![prompt_hook(
            "goal",
            "work until the task is verifiably done",
        )],
        deps,
    );
    manager.set_stop_loop_limit(2);
    manager.approve_all();
    let (shared, stages, config) = stop_stages(manager, 10);
    let dir = TempSession::new("hooks-stop-loop");
    let inference = ScriptedInference::new(vec![
        text_turn("working"),
        text_turn("still working"),
        text_turn("final answer"),
    ]);
    let mut session =
        AgentLoop::open(dir.path(), config, stages, Box::new(inference)).expect("open session");
    session.submit("finish the release");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    // Two blocks forced two extra inference rounds; the third consultation
    // exhausted the budget and let the turn exit.
    assert_eq!(
        session
            .events()
            .iter()
            .filter(|r| matches!(r.event, Event::Assistant { .. }))
            .count(),
        3
    );
    let evaluator_calls = seams.evaluator.calls();
    assert_eq!(evaluator_calls.len(), 3);
    assert_eq!(
        evaluator_calls[0].0,
        "work until the task is verifiably done"
    );
    assert!(
        serde_json::to_string(&evaluator_calls[0].1)
            .unwrap_or_default()
            .contains(r#""event":"stop""#)
    );
    let systems = system_texts(&session);
    assert!(
        systems.iter().any(|t| t.contains("tests still failing")),
        "block reasons recorded: {systems:?}"
    );
    assert!(
        systems
            .iter()
            .any(|t| t.contains("stop-hook loop budget exhausted")),
        "typed exhaustion notice: {systems:?}"
    );
    let manager = shared.inner().lock().await;
    let stops = payloads(manager.journal(), HookEvent::Stop);
    assert_eq!(stops.len(), 3);
    assert_eq!(
        serde_json::to_string(stops[0]).unwrap_or_default(),
        r#"{"event":"stop","turn":1}"#
    );
}

#[tokio::test]
async fn stop_hook_proceeds_when_criterion_met() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.evaluator.push(PolicyVerdict::Proceed);
    let mut manager = HookManager::new(vec![prompt_hook("goal", "done when green")], deps);
    manager.approve_all();
    let shared = SharedHooks::new(manager);
    let stages = common::stages_kit::stages(|s| s.stop_hook = Box::new(shared.stop_stage()));
    let dir = TempSession::new("hooks-stop-proceed");
    let inference = ScriptedInference::new(vec![text_turn("done")]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("work");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert!(
        !system_texts(&session)
            .iter()
            .any(|t| t.contains("stop-hook loop budget exhausted"))
    );
    let manager = shared.inner().lock().await;
    assert_eq!(payloads(manager.journal(), HookEvent::Stop).len(), 1);
}

#[tokio::test]
async fn stop_hook_failure_is_loud_fail_open() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.evaluator.push_err("LLM unavailable");
    let mut manager = HookManager::new(vec![prompt_hook("goal", "done when green")], deps);
    manager.approve_all();
    let shared = SharedHooks::new(manager);
    let stages = common::stages_kit::stages(|s| s.stop_hook = Box::new(shared.stop_stage()));
    let dir = TempSession::new("hooks-stop-fail");
    let inference = ScriptedInference::new(vec![text_turn("done")]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("work");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let systems = system_texts(&session);
    assert!(
        systems
            .iter()
            .any(|t| t.contains("stop hook failed") && t.contains("proceed")),
        "failure surfaces loudly: {systems:?}"
    );
}

#[tokio::test]
async fn loop_budget_persists_across_runs() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.evaluator.push(PolicyVerdict::ContinueWorking {
        reason: "one".to_owned(),
    });
    seams.evaluator.push(PolicyVerdict::ContinueWorking {
        reason: "two".to_owned(),
    });
    seams.evaluator.push(PolicyVerdict::Proceed);
    seams.evaluator.push(PolicyVerdict::ContinueWorking {
        reason: "three".to_owned(),
    });
    let mut manager = HookManager::new(vec![prompt_hook("goal", "done when green")], deps);
    manager.set_stop_loop_limit(2);
    manager.approve_all();
    let (shared, stages, config) = stop_stages(manager, 10);
    let dir = TempSession::new("hooks-stop-persist");
    let mut session = AgentLoop::open(
        dir.path(),
        config,
        stages,
        Box::new(ScriptedInference::new(vec![
            text_turn("a"),
            text_turn("b"),
            text_turn("c"),
        ])),
    )
    .expect("open session");
    session.submit("first");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let exhausted_after_first = {
        let manager = shared.inner().lock().await;
        manager.stop_budget_is_exhausted()
    };
    assert!(exhausted_after_first);

    // Run 2: the evaluator wants one more iteration ("three"), but the
    // budget spent in run 1 persists — the block is denied with the
    // typed exhaustion notice and the turn exits immediately.
    session.set_inference(Box::new(ScriptedInference::new(vec![text_turn("d")])));
    session.submit("second");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let second = system_texts(&session);
    let exhaustions = second
        .iter()
        .filter(|t| t.contains("stop-hook loop budget exhausted"))
        .count();
    assert_eq!(
        exhaustions, 1,
        "run-2 block denied by the spent budget: {second:?}"
    );
    assert!(
        !second.iter().any(|t| t.contains("three")),
        "the exhausted block reason must NOT re-prompt: {second:?}"
    );
    assert_eq!(seams.evaluator.calls().len(), 4);
}
