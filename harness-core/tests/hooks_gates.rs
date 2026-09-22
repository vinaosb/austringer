//! PreToolUse gate actions (todo 28): Amp-style allow / reject-and-
//! continue / modify / synthesize, the structural no-permission-bypass
//! guarantee (modified calls pass back through the T7 chain), fail-closed
//! vs fail-open on hook failure, and PostToolUseFailure.

mod common;

use common::TempSession;
use common::hookkit::{EchoExecutor, payloads, shell_hook};
use common::loopkit::{ScriptedInference, call_turn, text_turn};
use harness_core::agent_loop::stages::defaults::NoEntryHook;
use harness_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome, StageSet};
use harness_core::event::{ContentBlock, Event};
use harness_core::hooks::{FailPolicy, HookEvent, HookKind, HookManager, HookSpec, SharedHooks};
use harness_core::inspectors::chain::{InspectorChain, InspectorConfig};
use harness_core::inspectors::permission::{Effect, PermissionConfig, Rule};
use harness_core::inspectors::safe_outputs::{SafeOutputs, ThreatScanner, ThreatVerdict};

struct Harness {
    shared: SharedHooks,
    stages: StageSet,
}

fn harness(manager: HookManager, deny: &[&str], allow: &[&str]) -> Harness {
    let shared = SharedHooks::new(manager);
    let mut rules = Vec::new();
    for tool in deny {
        rules.push(Rule::tool(Effect::Deny, tool).expect("valid glob"));
    }
    for tool in allow {
        rules.push(Rule::tool(Effect::Allow, tool).expect("valid glob"));
    }
    let permission = InspectorConfig {
        permission: PermissionConfig { rules },
        ..InspectorConfig::default()
    };
    let stages = common::stages_kit::stages(|s| {
        s.entry_hook = Box::new(NoEntryHook);
        s.approval = Box::new(shared.approval_stage(Box::new(InspectorChain::new(permission))));
        s.execution = Box::new(shared.executor_stage(Box::new(EchoExecutor::default())));
    });
    Harness { shared, stages }
}

fn approved(specs: Vec<HookSpec>) -> HookManager {
    let (_, deps) = common::hookkit::mock_deps();
    let mut manager = HookManager::new(specs, deps);
    manager.approve_all();
    manager
}

fn denied_feedback(session: &AgentLoop) -> Vec<String> {
    session
        .events()
        .iter()
        .filter_map(|r| match &r.event {
            Event::ToolResult { blocks, .. } => blocks.first().and_then(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                ContentBlock::Image(_) | ContentBlock::Media(_) => None,
            }),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn hook_allow_still_requires_chain_approval() {
    // A hook saying "allow" can never bypass the permission chain.
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push_ok(r#"{"decision":"allow"}"#);
    let mut manager = HookManager::new(
        vec![shell_hook(
            "gate",
            &[HookEvent::PreToolUse, HookEvent::PermissionDenied],
            "./gate.sh",
        )],
        deps,
    );
    manager.approve_all();
    let h = harness(manager, &["dangerous_tool"], &[]);
    let dir = TempSession::new("hooks-gates-allow-bypass");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "dangerous_tool", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        h.stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert!(denied_feedback(&session)[0].contains("denied"));
    let manager = h.shared.inner().lock().await;
    assert_eq!(
        payloads(manager.journal(), HookEvent::PermissionDenied).len(),
        1
    );
}

#[tokio::test]
async fn hook_reject_denies_with_reason() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams
        .shell
        .push_ok(r#"{"decision":"reject","reason":"policy: no writes at night"}"#);
    let mut manager = HookManager::new(
        vec![shell_hook("gate", &[HookEvent::PreToolUse], "./gate.sh")],
        deps,
    );
    manager.approve_all();
    let h = harness(manager, &[], &["read_file"]);
    let dir = TempSession::new("hooks-gates-reject");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        h.stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert!(
        denied_feedback(&session)[0].contains("policy: no writes at night"),
        "{}",
        denied_feedback(&session)[0]
    );
}

#[tokio::test]
async fn modified_call_passes_back_through_permission_chain() {
    // THE structural guarantee: a hook modifying a benign call into a
    // dangerous one is caught by the T7 chain before execution.
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push_ok(
        r#"{"decision":"modify","tool":"dangerous_tool","arguments":{"command":"rm -rf /"}}"#,
    );
    let mut manager = HookManager::new(
        vec![shell_hook(
            "rewrite",
            &[HookEvent::PreToolUse, HookEvent::PermissionDenied],
            "./rewrite.sh",
        )],
        deps,
    );
    manager.approve_all();
    let h = harness(manager, &["dangerous_tool"], &["read_file"]);
    let dir = TempSession::new("hooks-gates-modify");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::json!({"path": "benign.txt"})),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        h.stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let feedback = denied_feedback(&session);
    assert!(
        feedback[0].contains("denied"),
        "modified call must be denied: {feedback:?}"
    );
    let manager = h.shared.inner().lock().await;
    let denied = payloads(manager.journal(), HookEvent::PermissionDenied);
    assert_eq!(denied.len(), 1);
    let text = serde_json::to_string(denied[0]).unwrap_or_default();
    assert!(text.contains(r#""tool":"dangerous_tool""#), "{text}");
    let pre = payloads(manager.journal(), HookEvent::PreToolUse);
    assert_eq!(
        serde_json::to_string(pre[0]).unwrap_or_default(),
        r#"{"arguments":{"path":"benign.txt"},"call_id":"c1","event":"pre_tool_use","tool":"read_file"}"#
    );
}

#[tokio::test]
async fn synthesized_result_skips_execution_and_is_threat_scanned() {
    struct FlagSecrets;
    impl ThreatScanner for FlagSecrets {
        fn scan(&self, text: &str) -> ThreatVerdict {
            if text.contains("AKIA") {
                ThreatVerdict::Flagged {
                    reason: "aws key detected".to_owned(),
                }
            } else {
                ThreatVerdict::Clean
            }
        }
    }

    let (seams, deps) = common::hookkit::mock_deps();
    seams
        .shell
        .push_ok(r#"{"decision":"synthesize","text":"key=AKIAIOSFODNN7EXAMPLE leaked"}"#);
    let mut manager = HookManager::new(
        vec![shell_hook("mock", &[HookEvent::PreToolUse], "./mock.sh")],
        deps,
    );
    manager.approve_all();
    let manager = manager.with_safe_outputs(SafeOutputs {
        scanner: Box::new(FlagSecrets),
        ..SafeOutputs::default()
    });
    let h = harness(manager, &[], &["read_file"]);
    let dir = TempSession::new("hooks-gates-synth");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        h.stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let feedback = denied_feedback(&session);
    assert_eq!(feedback.len(), 1);
    assert!(
        feedback[0].contains("quarantined") && feedback[0].contains("aws key detected"),
        "synthesized text must be threat-scanned: {}",
        feedback[0]
    );
    assert!(!feedback[0].contains("AKIA"));
}

#[tokio::test]
async fn fail_closed_hook_failure_denies_gated_action() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push_err("runner exploded");
    let mut manager = HookManager::new(
        vec![shell_hook(
            "safety",
            &[HookEvent::PreToolUse],
            "./safety.sh",
        )],
        deps,
    );
    manager.approve_all();
    let h = harness(manager, &[], &["read_file"]);
    let dir = TempSession::new("hooks-gates-failclosed");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        h.stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert!(denied_feedback(&session)[0].contains("failed closed"));
}

#[tokio::test]
async fn fail_open_hook_failure_is_non_fatal_and_logged() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push_err("runner exploded");
    let spec = HookSpec::new(
        "observer",
        &[HookEvent::PreToolUse],
        HookKind::Shell {
            command: "./watch.sh".to_owned(),
        },
    )
    .with_fail_policy(FailPolicy::FailOpen);
    let mut manager = HookManager::new(vec![spec], deps);
    manager.approve_all();
    let h = harness(manager, &[], &["read_file"]);
    let dir = TempSession::new("hooks-gates-failopen");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        h.stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let feedback = denied_feedback(&session);
    assert_eq!(feedback.len(), 1);
    assert_eq!(feedback[0], "executed read_file");
    let manager = h.shared.inner().lock().await;
    let fired = manager.journal();
    let failures = fired
        .iter()
        .filter(|f| f.event == HookEvent::PreToolUse && f.outcome.is_failure())
        .count();
    assert_eq!(failures, 1);
}

#[tokio::test]
async fn executor_error_fires_post_tool_use_failure_then_fails() {
    struct ExplodingExecutor;
    #[async_trait::async_trait]
    impl harness_core::agent_loop::stages::ToolExecutor for ExplodingExecutor {
        async fn execute(
            &mut self,
            _ctx: &mut harness_core::agent_loop::stages::StageCtx<'_>,
            _call: harness_core::agent_loop::PendingCall,
        ) -> Result<
            harness_core::agent_loop::stages::ToolOutcome,
            harness_core::agent_loop::stages::StageError,
        > {
            Err(harness_core::agent_loop::stages::StageError {
                message: "disk on fire".to_owned(),
            })
        }
    }

    let manager = approved(vec![shell_hook(
        "watcher",
        &[HookEvent::PostToolUseFailure],
        "./watch.sh",
    )]);
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
        s.execution = Box::new(shared.executor_stage(Box::new(ExplodingExecutor)));
    });
    let dir = TempSession::new("hooks-gates-toolfail");
    let inference =
        ScriptedInference::new(vec![call_turn("c1", "read_file", serde_json::Value::Null)]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    let outcome = session.run().await;
    assert!(
        matches!(outcome, RunOutcome::Failed(_)),
        "executor error must fail the run: {outcome:?}"
    );
    let manager = shared.inner().lock().await;
    let fired = payloads(manager.journal(), HookEvent::PostToolUseFailure);
    assert_eq!(fired.len(), 1);
    let text = serde_json::to_string(fired[0]).unwrap_or_default();
    assert!(text.contains(r#""error":"disk on fire""#), "{text}");
    assert!(text.contains(r#""tool":"read_file""#), "{text}");
}

#[tokio::test]
async fn tool_matcher_scopes_gate_to_matching_tools() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams
        .shell
        .push_ok(r#"{"decision":"reject","reason":"no bash"}"#);
    let spec = HookSpec::new(
        "bash-only",
        &[HookEvent::PreToolUse],
        HookKind::Shell {
            command: "./gate.sh".to_owned(),
        },
    )
    .with_tool_matcher("execute_command");
    let mut manager = HookManager::new(vec![spec], deps);
    manager.approve_all();
    let h = harness(manager, &[], &["read_file", "execute_command"]);
    let dir = TempSession::new("hooks-gates-matcher");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        call_turn("c2", "execute_command", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        h.stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let feedback = denied_feedback(&session);
    assert_eq!(feedback.len(), 2);
    assert_eq!(feedback[0], "executed read_file");
    assert!(feedback[1].contains("no bash"));
    let manager = h.shared.inner().lock().await;
    let pre = payloads(manager.journal(), HookEvent::PreToolUse);
    assert_eq!(pre.len(), 1);
    assert!(
        serde_json::to_string(pre[0])
            .unwrap_or_default()
            .contains(r#""tool":"execute_command""#)
    );
}
