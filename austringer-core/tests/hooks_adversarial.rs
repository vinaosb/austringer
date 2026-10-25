//! Adversarial hooks (todo 28): timeout caps on hung hooks (the core
//! probe — deterministic under paused time), malformed hook outputs
//! (typed, per fail-policy), untrusted synthesized text bounded by the
//! output budget, unicode payloads, and cancel-mid-hook cleanliness.

mod common;

use austringer_core::agent_loop::stages::defaults::NoEntryHook;
use austringer_core::agent_loop::{AgentLoop, LoopConfig, RunOutcome, StageSet};
use austringer_core::event::{ContentBlock, Event};
use austringer_core::hooks::{FailPolicy, HookEvent, HookKind, HookManager, HookSpec, SharedHooks};
use austringer_core::inspectors::chain::{InspectorChain, InspectorConfig};
use austringer_core::inspectors::permission::{Effect, PermissionConfig, Rule};
use common::TempSession;
use common::hookkit::{EchoExecutor, ShellScript, payloads, shell_hook};
use common::loopkit::{ScriptedInference, call_turn, text_turn};
use std::sync::Arc;

fn gate_stages(manager: HookManager, allow: &[&str]) -> (SharedHooks, StageSet) {
    let shared = SharedHooks::new(manager);
    let permission = InspectorConfig {
        permission: PermissionConfig {
            rules: allow
                .iter()
                .map(|tool| Rule::tool(Effect::Allow, tool).expect("valid glob"))
                .collect(),
        },
        ..InspectorConfig::default()
    };
    let stages = common::stages_kit::stages(|s| {
        s.entry_hook = Box::new(NoEntryHook);
        s.approval = Box::new(shared.approval_stage(Box::new(InspectorChain::new(permission))));
        s.execution = Box::new(shared.executor_stage(Box::new(EchoExecutor::default())));
    });
    (shared, stages)
}

fn result_texts(session: &AgentLoop) -> Vec<String> {
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

#[tokio::test(start_paused = true)]
async fn hung_hook_hits_timeout_cap_and_fails_closed() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push(ShellScript::Hung);
    let mut manager = HookManager::new(
        vec![shell_hook("safety", &[HookEvent::PreToolUse], "./hang.sh").with_timeout_secs(2)],
        deps,
    );
    manager.approve_all();
    let (shared, stages) = gate_stages(manager, &["read_file"]);
    let dir = TempSession::new("hooks-adv-hung-closed");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let results = result_texts(&session);
    assert!(results[0].contains("timed out"), "{results:?}");
    assert!(results[0].contains("failed closed"), "{results:?}");
    let manager = shared.inner().lock().await;
    assert!(manager.journal().iter().any(|f| f.outcome.is_failure()));
}

#[tokio::test(start_paused = true)]
async fn hung_hook_fail_open_proceeds_to_chain() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push(ShellScript::Hung);
    let spec = HookSpec::new(
        "watcher",
        &[HookEvent::PreToolUse],
        HookKind::Shell {
            command: "./hang.sh".to_owned(),
        },
    )
    .with_timeout_secs(1)
    .with_fail_policy(FailPolicy::FailOpen);
    let mut manager = HookManager::new(vec![spec], deps);
    manager.approve_all();
    let (shared, stages) = gate_stages(manager, &["read_file"]);
    let dir = TempSession::new("hooks-adv-hung-open");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    assert_eq!(result_texts(&session), vec!["executed read_file"]);
    let manager = shared.inner().lock().await;
    assert!(
        manager.journal()[0].outcome.is_failure(),
        "fail-open timeout must still journal the typed failure"
    );
}

#[tokio::test]
async fn malformed_hook_output_is_typed_and_per_policy() {
    for (fail_policy, expect_denied) in [
        (FailPolicy::FailClosed, true),
        (FailPolicy::FailOpen, false),
    ] {
        let (seams, _) = common::hookkit::mock_deps();
        seams.shell.push_ok("this is {{{ not json");
        seams.shell.push_ok(r#"{"decision":"maybe-so"}"#);
        seams.shell.push_ok(r#"{"decision":"modify"}"#);
        seams.shell.push(ShellScript::Exit(2));
        let spec = HookSpec::new(
            "flaky",
            &[HookEvent::PreToolUse],
            HookKind::Shell {
                command: "./flaky.sh".to_owned(),
            },
        )
        .with_fail_policy(fail_policy);

        for round in 0..4 {
            let deps = austringer_core::hooks::HookDeps {
                shell: Box::new(seams.shell.clone()),
                http: Box::new(seams.transport.clone()),
                mcp: Box::new(seams.mcp.clone()),
                evaluator: Box::new(seams.evaluator.clone()),
            };
            let mut manager = HookManager::new(vec![spec.clone()], deps);
            manager.approve_all();
            let (shared, stages) = gate_stages(manager, &["read_file"]);
            let dir = TempSession::new(&format!("hooks-adv-malformed-{}-{round}", expect_denied));
            let inference = ScriptedInference::new(vec![
                call_turn("c1", "read_file", serde_json::Value::Null),
                text_turn("stopped"),
            ]);
            let mut session = AgentLoop::open(
                dir.path(),
                LoopConfig::default(),
                stages,
                Box::new(inference),
            )
            .expect("open session");
            session.submit("go");
            assert_eq!(session.run().await, RunOutcome::TurnComplete);
            let results = result_texts(&session);
            assert_eq!(results.len(), 1);
            let denied = results[0].contains("denied") || results[0].contains("failed closed");
            assert_eq!(
                denied, expect_denied,
                "round {round} ({results:?}) for policy {fail_policy:?}"
            );
            assert!(
                shared.inner().lock().await.journal()[0]
                    .outcome
                    .is_failure(),
                "round {round}: malformed output must journal a typed failure"
            );
        }
    }
}

#[tokio::test]
async fn synthesized_result_is_bounded_by_output_budget() {
    let (seams, deps) = common::hookkit::mock_deps();
    let huge = "x".repeat(20_000);
    seams.shell.push_ok(
        &serde_json::json!({
            "decision": "synthesize",
            "text": huge
        })
        .to_string(),
    );
    let mut manager = HookManager::new(
        vec![shell_hook("mock", &[HookEvent::PreToolUse], "./mock.sh")],
        deps,
    );
    manager.approve_all();
    let (shared, stages) = gate_stages(manager, &["read_file"]);
    let dir = TempSession::new("hooks-adv-bounded");
    let inference = ScriptedInference::new(vec![
        call_turn("c1", "read_file", serde_json::Value::Null),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let results = result_texts(&session);
    assert_eq!(results.len(), 1);
    assert!(
        results[0].contains("output truncated"),
        "oversized synthesis must carry the omission marker: {} chars",
        results[0].len()
    );
    assert!(results[0].chars().count() < 20_000);
    assert_eq!(
        shared.inner().lock().await.journal().len(),
        1,
        "the synthesizing hook fired exactly once"
    );
}

#[tokio::test]
async fn unicode_prompts_and_payloads_survive_the_full_path() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams
        .shell
        .push_ok(r#"{"additional_context":"追加コンテキスト 🦀"}"#);
    let mut manager = HookManager::new(
        vec![shell_hook(
            "i18n",
            &[HookEvent::UserPromptSubmit],
            "./国際化.sh",
        )],
        deps,
    );
    manager.approve_all();
    let shared = SharedHooks::new(manager);
    let stages = common::stages_kit::stages(|s| s.entry_hook = Box::new(shared.entry_stage()));
    let dir = TempSession::new("hooks-adv-unicode");
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(ScriptedInference::new(vec![text_turn("ok")])),
    )
    .expect("open session");
    session.submit("プロンプト 🎌");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);

    let manager = shared.inner().lock().await;
    let fired = payloads(manager.journal(), HookEvent::UserPromptSubmit);
    assert_eq!(
        serde_json::to_string(fired[0]).unwrap_or_default(),
        "{\"event\":\"user_prompt_submit\",\"prompt\":\"プロンプト 🎌\"}"
    );
    assert_eq!(seams.shell.calls()[0].0, "./国際化.sh");
}

#[tokio::test]
async fn cancel_mid_hook_leaves_no_partial_journal_and_resumes_clean() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push(ShellScript::Hung);
    let mut manager = HookManager::new(
        vec![
            shell_hook("slow-augment", &[HookEvent::UserPromptSubmit], "./slow.sh")
                .with_timeout_secs(600),
        ],
        deps,
    );
    manager.approve_all();
    let shared = SharedHooks::new(manager);
    let stages = common::stages_kit::stages(|s| s.entry_hook = Box::new(shared.entry_stage()));
    let dir = TempSession::new("hooks-adv-cancel");
    let config = LoopConfig {
        cancel_grace: std::time::Duration::from_millis(50),
        ..LoopConfig::default()
    };
    let session = Arc::new(tokio::sync::Mutex::new(
        AgentLoop::open(
            dir.path(),
            config,
            stages,
            Box::new(ScriptedInference::new(vec![text_turn("a"), text_turn("b")])),
        )
        .expect("open session"),
    ));

    let cancel_handle = session.lock().await.cancel_handle();
    {
        let mut guard = session.lock().await;
        guard.submit("work");
    }
    let handle = session.clone();
    let runner = tokio::spawn(async move {
        let mut guard = handle.lock().await;
        guard.run().await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel_handle.cancel();
    let outcome = runner.await.expect("runner join");
    assert!(
        matches!(outcome, RunOutcome::Cancelled(_)),
        "expected cancel, got {outcome:?}"
    );

    {
        let manager = shared.inner().lock().await;
        let fired = payloads(manager.journal(), HookEvent::UserPromptSubmit);
        assert!(
            fired.is_empty(),
            "a dropped mid-flight hook leaves no journal entry"
        );
    }

    // Resume = reopening the session directory (a fresh loop/token);
    // the hung hook now answers and the loop completes cleanly.
    seams
        .shell
        .push_ok(r#"{"additional_context":"back online"}"#);
    let resume_stages =
        common::stages_kit::stages(|s| s.entry_hook = Box::new(shared.entry_stage()));
    let mut resumed = AgentLoop::open(
        dir.path(),
        LoopConfig {
            cancel_grace: std::time::Duration::from_millis(50),
            ..LoopConfig::default()
        },
        resume_stages,
        Box::new(ScriptedInference::new(vec![text_turn("b")])),
    )
    .expect("reopen session");
    resumed.submit("retry");
    assert_eq!(resumed.run().await, RunOutcome::TurnComplete);
    let manager = shared.inner().lock().await;
    let fired = payloads(manager.journal(), HookEvent::UserPromptSubmit);
    assert_eq!(fired.len(), 1);
}

#[tokio::test]
async fn journal_payload_content_is_asserted_not_just_firing() {
    // misleading_success probe: assert the exact JSON payload bytes, not
    // merely that an event fired.
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push_ok(r#"{"decision":"allow"}"#);
    let mut manager = HookManager::new(
        vec![shell_hook("gate", &[HookEvent::PreToolUse], "./gate.sh")],
        deps,
    );
    manager.approve_all();
    let (shared, stages) = gate_stages(manager, &["read_file"]);
    let dir = TempSession::new("hooks-adv-payload");
    let inference = ScriptedInference::new(vec![
        call_turn(
            "c1",
            "read_file",
            serde_json::json!({"path":"日本語/ファイル.txt"}),
        ),
        text_turn("stopped"),
    ]);
    let mut session = AgentLoop::open(
        dir.path(),
        LoopConfig::default(),
        stages,
        Box::new(inference),
    )
    .expect("open session");
    session.submit("go");
    assert_eq!(session.run().await, RunOutcome::TurnComplete);
    let manager = shared.inner().lock().await;
    let fired = payloads(manager.journal(), HookEvent::PreToolUse);
    assert_eq!(fired.len(), 1);
    let golden = serde_json::to_string(fired[0]).unwrap_or_default();
    assert_eq!(
        golden,
        "{\"arguments\":{\"path\":\"日本語/ファイル.txt\"},\"call_id\":\"c1\",\"event\":\"pre_tool_use\",\"tool\":\"read_file\"}"
    );
    let sent_to_hook = seams.shell.calls()[0].1.clone();
    assert_eq!(
        serde_json::to_string(&sent_to_hook).unwrap_or_default(),
        golden
    );
}
