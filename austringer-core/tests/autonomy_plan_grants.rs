//! Todo 16 (ecosystem alignment) acceptance: entering plan mode
//! SUSPENDS every session-scoped grant (GrantScope::Session) until
//! approval, while forever grants stay honored — the matrix-level
//! contract the daemon's plan gate composes with.

mod common;

use std::sync::Arc;

use austringer_core::autonomy::{
    AutonomyHandle, AutonomyMatrix, AutonomyMode, AutonomyParts, autonomy_chain,
};
use austringer_core::inspectors::chain::{ChainOutcome, InspectorChain, InspectorConfig};
use austringer_core::inspectors::store::{
    Grant, GrantScope, MemoryStore, PermissionStore, RefuseAsks,
};
use common::autonomykit::{ScriptedIntent, ScriptedRisk};
use serde_json::json;

fn chain_over(
    matrix: AutonomyMatrix,
    store: Arc<dyn PermissionStore>,
) -> (AutonomyHandle, InspectorChain) {
    let handle = AutonomyHandle::new(matrix);
    let chain = autonomy_chain(AutonomyParts {
        matrix: handle.shared(),
        store,
        inspectors: InspectorConfig::default(),
        ask: Box::new(RefuseAsks),
        intent: Box::new(ScriptedIntent::new(vec![])),
        risk: Box::new(ScriptedRisk::new(vec![])),
        classifier: austringer_core::autonomy::ClassifierConfig::default(),
    });
    (handle, chain)
}

fn session_grant(tool: &str, target: &str) -> Grant {
    Grant {
        tool: tool.to_owned(),
        target: Some(target.to_owned()),
        scope: GrantScope::Session,
    }
}

#[tokio::test]
async fn entering_plan_mode_suspends_session_grants() {
    // Chat mode (read toggle off) + a session grant on view_window:
    // the grant is authoritative until plan mode suspends it.
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    store
        .record(&session_grant("view_window", "src/lib.rs"))
        .expect("record");
    let (handle, mut chain) = chain_over(AutonomyMatrix::for_mode(AutonomyMode::Chat), store);

    let read = common::inspectorkit::call("view_window", json!({"path": "src/lib.rs"}));
    match chain.evaluate(&read).await.expect("grant honored") {
        ChainOutcome::Allow { .. } => {}
        other => panic!("session grant should allow the read, got {other:?}"),
    }

    handle.write(|matrix| matrix.enter_plan());
    match chain.evaluate(&read).await.expect("plan suspension") {
        ChainOutcome::Block { feedback } => {
            assert!(
                feedback.contains("ask was refused"),
                "suspension falls to the mode ask: {feedback}"
            );
        }
        other => panic!("plan mode must suspend the session grant, got {other:?}"),
    }
}

#[tokio::test]
async fn approval_restores_the_suspended_session_grant() {
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    store
        .record(&session_grant("view_window", "src/lib.rs"))
        .expect("record");
    let (handle, mut chain) = chain_over(AutonomyMatrix::for_mode(AutonomyMode::Chat), store);

    handle.write(|matrix| matrix.enter_plan());
    // Approval flips autonomy back per the matrix: the mode resets (the
    // plan pairing clears with it) and the suspended grant returns.
    handle.write(|matrix| matrix.set_mode(AutonomyMode::Chat));

    let read = common::inspectorkit::call("view_window", json!({"path": "src/lib.rs"}));
    match chain.evaluate(&read).await.expect("restored") {
        ChainOutcome::Allow { .. } => {}
        other => panic!("approval must restore the session grant, got {other:?}"),
    }
}

#[tokio::test]
async fn forever_grants_survive_plan_mode() {
    let store: Arc<dyn PermissionStore> = Arc::new(MemoryStore::new());
    store
        .record(&Grant {
            tool: "view_window".to_owned(),
            target: Some("src/lib.rs".to_owned()),
            scope: GrantScope::Forever,
        })
        .expect("record");
    let (handle, mut chain) = chain_over(AutonomyMatrix::for_mode(AutonomyMode::Chat), store);

    handle.write(|matrix| matrix.enter_plan());
    let read = common::inspectorkit::call("view_window", json!({"path": "src/lib.rs"}));
    match chain.evaluate(&read).await.expect("forever grant") {
        ChainOutcome::Allow { .. } => {}
        other => panic!("forever grants must survive plan mode, got {other:?}"),
    }
}

#[test]
fn matrix_honors_grant_encodes_the_suspension() {
    let mut matrix = AutonomyMatrix::for_mode(AutonomyMode::Approve);
    assert!(matrix.honors_grant(GrantScope::Session));
    assert!(matrix.honors_grant(GrantScope::Forever));
    matrix.enter_plan();
    assert!(
        !matrix.honors_grant(GrantScope::Session),
        "session grants suspend in plan mode"
    );
    assert!(
        matrix.honors_grant(GrantScope::Forever),
        "forever grants never suspend"
    );
    matrix.set_mode(AutonomyMode::Approve);
    assert!(
        matrix.honors_grant(GrantScope::Session),
        "approval restores session grants"
    );
}
