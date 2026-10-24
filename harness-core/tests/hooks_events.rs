//! Hook event set (todo 28): every lifecycle event fires with a
//! deterministic, sorted-key payload snapshot; UserPromptSubmit augments
//! the prompt through the real loop; a PostToolUse synthesizer replaces
//! tool output; SessionStart fires exactly once.

mod common;

use common::TempSession;
use common::hookkit::{EchoExecutor, OnceCompactor, payloads, shell_hook};
use common::loopkit::{ScriptedInference, call_turn, text_turn};
use harness_core::agent_loop::stages::defaults::NoEntryHook;
use harness_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome, StageSet};
use harness_core::event::{ContentBlock, Event};
use harness_core::hooks::{HookEvent, HookManager, HookSpec, SharedHooks};
use harness_core::inspectors::chain::{InspectorChain, InspectorConfig};
use harness_core::inspectors::permission::{Effect, PermissionConfig, Rule};

fn approved(specs: Vec<HookSpec>) -> HookManager {
    let (_, deps) = common::hookkit::mock_deps();
    let mut manager = HookManager::new(specs, deps);
    manager.approve_all();
    manager
}

fn entry_only(manager: HookManager) -> (SharedHooks, StageSet) {
    let shared = SharedHooks::new(manager);
    let stages = common::stages_kit::stages(|s| s.entry_hook = Box::new(shared.entry_stage()));
    (shared, stages)
}

#[tokio::test]
async fn session_start_fires_once_across_runs() {
    let manager = approved(vec![shell_hook(
        "boot",
        &[HookEvent::SessionStart],
        "echo hi",
    )]);
    let (shared, stages) = entry_only(manager);
    let dir = TempSession::new("hooks-events-once");
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(ScriptedInference::new(vec![text_turn("a"), text_turn("b")])),
    )
    .expect("open session");

    session.submit("q1");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    session.submit("q2");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let manager = shared.inner().lock().await;
    let starts = payloads(manager.journal(), HookEvent::SessionStart);
    assert_eq!(starts.len(), 1);
    assert_eq!(
        serde_json::to_string(starts[0]).unwrap_or_default(),
        r#"{"event":"session_start"}"#
    );
}

#[tokio::test]
async fn user_prompt_submit_hook_augments_prompt_through_loop() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams
        .shell
        .push_ok(r#"{"additional_context":"remember: deploys happen friday"}"#);
    let mut manager = HookManager::new(
        vec![shell_hook(
            "ctx",
            &[HookEvent::UserPromptSubmit],
            "./enrich.sh",
        )],
        deps,
    );
    manager.approve_all();
    let (shared, stages) = entry_only(manager);
    let dir = TempSession::new("hooks-events-augment");
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(ScriptedInference::new(vec![text_turn("ok")])),
    )
    .expect("open session");

    session.submit("deploy the service");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let user_text: Vec<String> = session
        .events()
        .iter()
        .filter_map(|r| match &r.event {
            Event::User { blocks } => blocks.first().and_then(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                ContentBlock::Image(_) | ContentBlock::Media(_) => None,
            }),
            _ => None,
        })
        .collect();
    assert_eq!(
        user_text,
        vec!["deploy the service\n\n[hooks]\nremember: deploys happen friday"]
    );
    let manager = shared.inner().lock().await;
    let fired = payloads(manager.journal(), HookEvent::UserPromptSubmit);
    assert_eq!(fired.len(), 1);
    assert_eq!(
        serde_json::to_string(fired[0]).unwrap_or_default(),
        r#"{"event":"user_prompt_submit","prompt":"deploy the service"}"#
    );
}

#[tokio::test]
async fn each_lifecycle_event_fires_with_deterministic_payload_snapshot() {
    let manager = approved(vec![shell_hook("observer", HookEvent::ALL, "./watch.sh")]);
    let shared = SharedHooks::new(manager);
    let permission = InspectorConfig {
        permission: PermissionConfig {
            rules: vec![Rule::tool(Effect::Allow, "read_file").expect("valid glob")],
        },
        ..InspectorConfig::default()
    };
    let stages = common::stages_kit::stages(|s| {
        s.entry_hook = Box::new(shared.entry_stage());
        s.approval = Box::new(shared.approval_stage(Box::new(InspectorChain::new(permission))));
        s.execution = Box::new(shared.executor_stage(Box::new(EchoExecutor::default())));
        s.compaction = Box::new(shared.compactor_stage(Box::new(OnceCompactor::new())));
    });

    let dir = TempSession::new("hooks-events-all");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::json!({"path": "a.txt"})),
        text_turn("done"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("start work");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    shared.notify("awaiting input").await;
    shared.subagent_start("researcher").await;
    shared.subagent_stop("researcher").await;
    shared.fire_session_end().await;

    let manager = shared.inner().lock().await;
    let journal = manager.journal();
    let expect = |event: HookEvent, want: &str| {
        let got = payloads(journal, event);
        assert_eq!(got.len(), 1, "event {event:?} fired {} times", got.len());
        assert_eq!(
            serde_json::to_string(got[0]).unwrap_or_default(),
            want,
            "payload for {event:?}"
        );
    };
    expect(HookEvent::SessionStart, r#"{"event":"session_start"}"#);
    expect(
        HookEvent::UserPromptSubmit,
        r#"{"event":"user_prompt_submit","prompt":"start work"}"#,
    );
    expect(
        HookEvent::PreToolUse,
        r#"{"arguments":{"path":"a.txt"},"call_id":"c1","event":"pre_tool_use","tool":"read_file"}"#,
    );
    expect(
        HookEvent::PostToolUse,
        r#"{"call_id":"c1","event":"post_tool_use","result":"executed read_file","tool":"read_file"}"#,
    );
    expect(HookEvent::PreCompact, r#"{"event":"pre_compact"}"#);
    expect(HookEvent::PostCompact, r#"{"event":"post_compact"}"#);
    expect(
        HookEvent::Notification,
        r#"{"event":"notification","message":"awaiting input"}"#,
    );
    expect(
        HookEvent::SubagentStart,
        r#"{"agent":"researcher","event":"subagent_start"}"#,
    );
    expect(
        HookEvent::SubagentStop,
        r#"{"agent":"researcher","event":"subagent_stop"}"#,
    );
    expect(HookEvent::SessionEnd, r#"{"event":"session_end"}"#);
}

#[tokio::test]
async fn permission_denied_fires_when_chain_denies() {
    let manager = approved(vec![shell_hook(
        "audit",
        &[HookEvent::PermissionDenied],
        "./audit.sh",
    )]);
    let shared = SharedHooks::new(manager);
    let permission = InspectorConfig {
        permission: PermissionConfig {
            rules: vec![Rule::tool(Effect::Deny, "dangerous_tool").expect("valid glob")],
        },
        ..InspectorConfig::default()
    };
    let stages = common::stages_kit::stages(|s| {
        s.entry_hook = Box::new(NoEntryHook);
        s.approval = Box::new(shared.approval_stage(Box::new(InspectorChain::new(permission))));
    });
    let dir = TempSession::new("hooks-events-permdenied");
    let inference = ScriptedInference::new(vec![
        call_turn("c9", "dangerous_tool", serde_json::Value::Null),
        text_turn("okay"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("try it");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let manager = shared.inner().lock().await;
    let fired = payloads(manager.journal(), HookEvent::PermissionDenied);
    assert_eq!(fired.len(), 1);
    let text = serde_json::to_string(fired[0]).unwrap_or_default();
    assert!(text.contains(r#""event":"permission_denied""#), "{text}");
    assert!(text.contains(r#""tool":"dangerous_tool""#), "{text}");
}

#[tokio::test]
async fn post_tool_use_synthesizer_replaces_output_before_context() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams
        .shell
        .push_ok(r#"{"decision":"replace","text":"[summary] 3 files changed"}"#);
    let mut manager = HookManager::new(
        vec![shell_hook(
            "redact",
            &[HookEvent::PostToolUse],
            "./redact.sh",
        )],
        deps,
    );
    manager.approve_all();
    let shared = SharedHooks::new(manager);
    let permission = InspectorConfig {
        permission: PermissionConfig {
            rules: vec![Rule::tool(Effect::Allow, "read_file").expect("valid glob")],
        },
        ..InspectorConfig::default()
    };
    let stages = common::stages_kit::stages(|s| {
        s.entry_hook = Box::new(NoEntryHook);
        s.approval = Box::new(shared.approval_stage(Box::new(InspectorChain::new(permission))));
        s.execution = Box::new(shared.executor_stage(Box::new(EchoExecutor::default())));
    });
    let dir = TempSession::new("hooks-events-synth");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::json!({"path": "a.txt"})),
        text_turn("done"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("read it");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let results: Vec<String> = session
        .events()
        .iter()
        .filter_map(|r| match &r.event {
            Event::ToolResult { blocks, .. } => blocks.first().and_then(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                ContentBlock::Image(_) | ContentBlock::Media(_) => None,
            }),
            _ => None,
        })
        .collect();
    assert_eq!(results, vec!["[summary] 3 files changed"]);
}
