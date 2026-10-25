//! Mode-switch approval-behavior tests (todo 8 acceptance): switching the
//! autonomy mode changes what asks and what runs — while the serialized
//! tool schema stays byte-identical — plus the destructive floor, the
//! softDeny probe, grant precedence, denial escalation, and the headless
//! default.

mod common;

use std::sync::Arc;

use austringer_core::autonomy::{
    AutonomyHandle, AutonomyMatrix, AutonomyMode, AutonomyParts, Capability, autonomy_chain,
};
use austringer_core::inspectors::chain::{ChainOutcome, InspectorChain, InspectorConfig};
use austringer_core::inspectors::store::{
    AskResolution, Grant, GrantScope, MemoryStore, PermissionStore, RefuseAsks,
};
use austringer_core::tools::{IoCap, ToolRegistry};
use common::autonomykit::{ScriptedIntent, ScriptedRisk};
use common::inspectorkit::{CaptureAsks, call};
use serde_json::json;

fn parts(matrix: AutonomyMatrix) -> (AutonomyHandle, InspectorChain) {
    let handle = AutonomyHandle::new(matrix);
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store: store.clone(),
        inspectors: InspectorConfig::default(),
        ask: Box::new(RefuseAsks),
        intent: Box::new(ScriptedIntent::new(vec![
            austringer_core::autonomy::Intent::Benign,
        ])),
        risk: Box::new(ScriptedRisk::new(vec![
            austringer_core::autonomy::Risk::Low,
        ])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    (handle, chain)
}

#[tokio::test]
async fn mode_switches_change_approval_without_schema_churn() {
    let dir = common::TempSession::new("mode-schema");
    let registry = ToolRegistry::open(dir.path(), IoCap::default()).expect("registry");
    let pinned = registry.pin_current();
    let baseline = registry.render_base_schema(&pinned);

    let (handle, mut chat) = parts(AutonomyMatrix::for_mode(AutonomyMode::Chat));
    // Distinct commands per phase: the T7 repetition inspector must not
    // confuse a mode probe with a stuck loop.
    let npm = call("execute_command", json!({"command": "npm test"}));
    // Chat: everything asks; headless policy refuses the ask.
    match chat.evaluate(&npm).await.expect("chat") {
        ChainOutcome::Block { feedback } => assert!(feedback.contains("ask was refused")),
        other => panic!("chat should block, got {other:?}"),
    }
    assert_eq!(registry.render_base_schema(&pinned), baseline);

    handle.write(|matrix| matrix.set_mode(AutonomyMode::SmartApprove));
    // SmartApprove: exec toggle off -> classifier (mock says benign/low).
    match chat
        .evaluate(&call(
            "execute_command",
            json!({"command": "cargo fmt --check"}),
        ))
        .await
        .expect("smartapprove")
    {
        ChainOutcome::Allow { .. } => {}
        other => panic!("smartapprove should approve benign, got {other:?}"),
    }
    assert_eq!(registry.render_base_schema(&pinned), baseline);

    handle.write(|matrix| matrix.set_mode(AutonomyMode::Approve));
    match chat
        .evaluate(&call("execute_command", json!({"command": "cargo clippy"})))
        .await
        .expect("approve")
    {
        ChainOutcome::Block { feedback } => assert!(feedback.contains("ask was refused")),
        other => panic!("approve should ask (headless refuse), got {other:?}"),
    }
    assert_eq!(registry.render_base_schema(&pinned), baseline);

    // Auto with the exec toggle flipped on: capability layer-2 allow, no
    // classifier consult, no ask.
    handle.write(|matrix| {
        matrix.set_mode(AutonomyMode::Auto);
        matrix.toggle(Capability::Exec, true);
    });
    match chat
        .evaluate(&call(
            "execute_command",
            json!({"command": "cargo nextest run"}),
        ))
        .await
        .expect("auto+toggle")
    {
        ChainOutcome::Allow { .. } => {}
        other => panic!("auto with exec toggle should allow, got {other:?}"),
    }
    assert_eq!(registry.render_base_schema(&pinned), baseline);
}

#[tokio::test]
async fn smart_approve_approves_benign_reads_with_no_ask_and_no_consult() {
    // QA happy path 1: verdict chain = capability layer-2 allow -> no ask.
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::SmartApprove));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let intent = ScriptedIntent::new(vec![]);
    let risk = ScriptedRisk::new(vec![]);
    let asks = CaptureAsks::with(vec![]);
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(asks.clone()),
        intent: Box::new(intent),
        risk: Box::new(risk),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    let read = call("view_window", json!({"path": "src/main.rs"}));
    match chain.evaluate(&read).await.expect("read") {
        ChainOutcome::Allow { notice } => assert!(notice.is_none()),
        other => panic!("benign read should allow, got {other:?}"),
    }
    assert!(asks.asked().is_empty(), "layer-2 allow must not ask");
}

#[tokio::test]
async fn destructive_commands_ask_in_every_mode_including_auto() {
    for mode in AutonomyMode::ALL {
        let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(mode));
        let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
        let asks = CaptureAsks::with(vec![AskResolution::Refuse]);
        let mut chain = autonomy_chain(AutonomyParts {
            matrix: handle.shared(),
            store,
            inspectors: InspectorConfig::default(),
            ask: Box::new(asks.clone()),
            intent: Box::new(ScriptedIntent::new(vec![
                austringer_core::autonomy::Intent::Benign,
            ])),
            risk: Box::new(ScriptedRisk::new(vec![
                austringer_core::autonomy::Risk::Low,
            ])),
            classifier: austringer_core::autonomy::ClassifierConfig::default(),
        });
        let rm_rf = call(
            "execute_command",
            json!({"command": "rm -rf /tmp/important"}),
        );
        match chain.evaluate(&rm_rf).await.expect("evaluate") {
            ChainOutcome::Allow { .. } => panic!("{mode:?} auto-approved a destructive command"),
            ChainOutcome::Block { .. } => {
                assert_eq!(asks.asked().len(), 1, "{mode:?} must ask exactly once");
                let reason = asks.asked()[0].reason.to_lowercase();
                assert!(reason.contains("destructive"), "{mode:?} reason: {reason}");
            }
        }
    }
}

#[tokio::test]
async fn secret_writes_ask_even_when_edit_is_auto() {
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::Auto));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let asks = CaptureAsks::with(vec![AskResolution::Refuse]);
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(asks.clone()),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    let env_write = call("edit_lines", json!({"path": ".env", "edits": []}));
    match chain.evaluate(&env_write).await.expect("evaluate") {
        ChainOutcome::Block { feedback } => {
            let lower = feedback.to_lowercase();
            assert!(
                lower.contains("secret") || lower.contains("ask was refused"),
                "{feedback}"
            );
        }
        other => panic!("secret write must not pass, got {other:?}"),
    }
    assert_eq!(asks.asked().len(), 1, "the secret guard asked");
}

#[tokio::test]
async fn exact_harmful_intent_soft_denies_even_in_auto() {
    // softDeny probe: a command OUTSIDE the destructive floor patterns,
    // with the intent stage reporting the request literally asks for the
    // harmful thing -> typed soft-deny, before any ask.
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::Auto));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let asks = CaptureAsks::with(vec![AskResolution::GrantOnce]);
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(asks.clone()),
        intent: Box::new(ScriptedIntent::new(vec![
            austringer_core::autonomy::Intent::HarmfulExact,
        ])),
        risk: Box::new(ScriptedRisk::new(vec![
            austringer_core::autonomy::Risk::Low,
        ])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    let force_push = call(
        "execute_command",
        json!({"command": "git push --force origin main"}),
    );
    match chain.evaluate(&force_push).await.expect("evaluate") {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.to_lowercase().contains("harmful"), "{feedback}");
        }
        other => panic!("expected softDeny block, got {other:?}"),
    }
    assert!(asks.asked().is_empty(), "softDeny must not burn an ask");
}

#[tokio::test]
async fn stored_grants_bypass_mode_asks_but_not_the_destructive_floor() {
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::Chat));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    store
        .record(&Grant {
            tool: "execute_command".to_owned(),
            target: Some("cargo test".to_owned()),
            scope: GrantScope::Forever,
        })
        .expect("grant");
    store
        .record(&Grant {
            tool: "execute_command".to_owned(),
            target: Some("rm -rf /tmp/x".to_owned()),
            scope: GrantScope::Forever,
        })
        .expect("floor grant");
    let asks = CaptureAsks::with(vec![AskResolution::Refuse]);
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(asks.clone()),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    let granted = call("execute_command", json!({"command": "cargo test"}));
    match chain.evaluate(&granted).await.expect("granted") {
        ChainOutcome::Allow { .. } => {}
        other => panic!("stored grant should allow, got {other:?}"),
    }
    // The floor fires BEFORE the grant: destructive still asks.
    let destructive = call("execute_command", json!({"command": "rm -rf /tmp/x"}));
    assert!(matches!(
        chain.evaluate(&destructive).await.expect("floor"),
        ChainOutcome::Block { .. }
    ));
    assert_eq!(asks.asked().len(), 1);
}

#[tokio::test]
async fn repeated_denials_escalate_to_a_typed_block() {
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::SmartApprove));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    for _ in 0..3 {
        store
            .record_denial("execute_command", Some("cargo publish"))
            .expect("denials");
    }
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(RefuseAsks),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    match chain
        .evaluate(&call(
            "execute_command",
            json!({"command": "cargo publish"}),
        ))
        .await
        .expect("escalate")
    {
        ChainOutcome::Block { feedback } => {
            assert!(feedback.to_lowercase().contains("denied"), "{feedback}");
        }
        other => panic!("expected denial escalation block, got {other:?}"),
    }
}

#[tokio::test]
async fn headless_default_asks_and_refuses_everything() {
    // Default matrix (Chat, all toggles off) + RefuseAsks: no input means
    // no execution — even benign reads.
    let handle = AutonomyHandle::new(AutonomyMatrix::default());
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(RefuseAsks),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    let read = call("view_window", json!({"path": "README.md"}));
    match chain.evaluate(&read).await.expect("read") {
        ChainOutcome::Block { feedback } => assert!(feedback.contains("ask was refused")),
        other => panic!("headless default must refuse, got {other:?}"),
    }
}

#[tokio::test]
async fn unknown_tools_ask_rather_than_pass() {
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::Auto));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let asks = CaptureAsks::with(vec![AskResolution::Refuse]);
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(asks.clone()),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    let mystery = call("mcp_web_fetch", json!({"url": "https://example.com"}));
    match chain.evaluate(&mystery).await.expect("mystery") {
        ChainOutcome::Block { .. } => assert_eq!(asks.asked().len(), 1),
        other => panic!("unknown tool must ask, got {other:?}"),
    }
}

#[tokio::test]
async fn grant_memory_toggle_downgrades_always_to_once() {
    // GrantMemory ON (Approve preset): a GrantAlways answer persists.
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::Approve));
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store: store.clone(),
        inspectors: InspectorConfig::default(),
        ask: Box::new(CaptureAsks::with(vec![AskResolution::GrantAlways(
            GrantScope::Session,
        )])),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    chain.evaluate(&exec_("cargo build")).await.expect("first");
    assert_eq!(
        store.lookup("execute_command", Some("cargo build")),
        Some(GrantScope::Session)
    );
    // GrantMemory OFF: the same answer downgrades to once — nothing stored.
    let handle = AutonomyHandle::new(AutonomyMatrix::for_mode(AutonomyMode::Approve));
    handle.write(|matrix| matrix.toggle(Capability::GrantMemory, false));
    let fresh: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    let mut chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store: fresh.clone(),
        inspectors: InspectorConfig::default(),
        ask: Box::new(CaptureAsks::with(vec![AskResolution::GrantAlways(
            GrantScope::Session,
        )])),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    chain.evaluate(&exec_("cargo build")).await.expect("second");
    assert_eq!(fresh.lookup("execute_command", Some("cargo build")), None);
}

fn exec_(command: &str) -> austringer_core::agent_loop::PendingCall {
    call("execute_command", json!({"command": command}))
}
