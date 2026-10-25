//! Hook config trust review + hash pinning (todo 28): first use requires
//! review (inert until approved), approval pins the config content hash,
//! a modified config is inert until re-approved, and pins survive
//! restart via the serialized registry.

mod common;

use austringer_core::hooks::{HookEvent, HookManager, HookSpec, TrustCheck, TrustRegistry};
use common::hookkit::{payloads, shell_hook};

fn manager(specs: Vec<HookSpec>) -> HookManager {
    let (_, deps) = common::hookkit::mock_deps();
    HookManager::new(specs, deps)
}

#[tokio::test]
async fn unapproved_hook_is_inert_pending_review() {
    let (seams, deps) = common::hookkit::mock_deps();
    let hooks = manager(vec![shell_hook(
        "gate",
        &[HookEvent::Notification],
        "./gate.sh",
    )]);
    let mut hooks = hooks;
    hooks.notify("hello").await;
    assert!(
        seams.shell.calls().is_empty(),
        "unapproved hook must never run"
    );
    let journal = hooks.journal();
    let fired = payloads(journal, HookEvent::Notification);
    assert_eq!(fired.len(), 1);
    assert!(journal[0].outcome.is_inert_pending_review());
    let _ = deps;
}

#[tokio::test]
async fn approval_pins_hash_and_lets_hook_run() {
    let (seams, deps) = common::hookkit::mock_deps();
    seams.shell.push_ok(r#"{"decision":"allow"}"#);
    let mut hooks = HookManager::new(
        vec![shell_hook("gate", &[HookEvent::Notification], "./gate.sh")],
        deps,
    );
    hooks.approve("gate");
    hooks.notify("hello").await;
    assert_eq!(seams.shell.calls().len(), 1);
}

#[tokio::test]
async fn modified_config_is_inert_until_reapproved() {
    let (seams, deps) = common::hookkit::mock_deps();
    let mut hooks = HookManager::new(
        vec![shell_hook(
            "gate",
            &[HookEvent::Notification],
            "./benign.sh",
        )],
        deps,
    );
    hooks.approve("gate");
    hooks.notify("first").await;
    assert_eq!(seams.shell.calls().len(), 1);
    assert_eq!(seams.shell.calls()[0].0, "./benign.sh");
    let pinned_registry = hooks.trust_registry().clone();

    // Restart: the pinned registry travels, the config on disk changed.
    // Same name, different content = hash mismatch = inert until
    // re-review, even though the name was previously approved.
    let (seams2, deps2) = common::hookkit::mock_deps();
    let mut hooks = HookManager::new(
        vec![shell_hook(
            "gate",
            &[HookEvent::Notification],
            "./tampered.sh",
        )],
        deps2,
    );
    hooks.set_trust_registry(pinned_registry);
    hooks.notify("second").await;
    let journal = hooks.journal();
    assert_eq!(journal.len(), 1);
    assert!(
        journal[0].outcome.is_inert_modified(),
        "hash mismatch must inert the hook"
    );
    assert!(
        seams2.shell.calls().is_empty(),
        "tampered hook must never execute"
    );

    hooks.approve("gate");
    hooks.notify("third").await;
    assert_eq!(hooks.journal().len(), 2);
    assert!(!hooks.journal()[1].outcome.is_inert());
}

#[tokio::test]
async fn trust_pin_survives_restart_via_serialized_registry() {
    let mut registry = TrustRegistry::new();
    let original = shell_hook("gate", &[HookEvent::Notification], "./pinned.sh");
    let hash = austringer_core::hooks::spec_hash(&original);
    assert_eq!(registry.classify("gate", &hash), TrustCheck::PendingReview);
    registry.approve("gate", hash.clone());
    assert_eq!(registry.classify("gate", &hash), TrustCheck::Approved);

    // "Restart": the registry travels through its serialized form.
    let serialized = serde_json::to_string(&registry).expect("serialize registry");
    let restored: TrustRegistry = serde_json::from_str(&serialized).expect("restore registry");
    assert_eq!(restored.classify("gate", &hash), TrustCheck::Approved);

    // Same name, modified content after restart: re-review required.
    let tampered = shell_hook("gate", &[HookEvent::Notification], "./pinned-evil.sh");
    let tampered_hash = austringer_core::hooks::spec_hash(&tampered);
    assert!(matches!(
        restored.classify("gate", &tampered_hash),
        TrustCheck::Modified { .. }
    ));
}

#[tokio::test]
async fn approve_is_per_name_and_revocable() {
    let mut hooks = manager(vec![
        shell_hook("a", &[HookEvent::Notification], "./a.sh"),
        shell_hook("b", &[HookEvent::Notification], "./b.sh"),
    ]);
    hooks.approve("a");
    hooks.notify("x").await;
    let journal = hooks.journal();
    assert!(journal[0].outcome.ran());
    assert!(journal[1].outcome.is_inert_pending_review());

    hooks.revoke("a");
    hooks.notify("y").await;
    let journal = hooks.journal();
    assert!(journal[2].outcome.is_inert_pending_review());
}
