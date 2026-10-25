//! Policy layer tests (todo 26): declaration defaults, session pinning,
//! violation seam, escape gate, deny-wins merge.

#![cfg(test)]

use austringer_safety::policy::decl::{
    DenyGlob, EscapeGate, PinnedProfile, SandboxPolicy, SandboxTier, ViolationDecision,
    ViolationEvent, ViolationKind,
};
use austringer_safety::policy::merge::{PinError, attempt_change, merge_policies, restore};
use std::path::Path;

fn worktree_policy(root: &Path) -> SandboxPolicy {
    SandboxPolicy::for_worktree(root, &root.join("settings.toml"))
}

#[test]
fn worktree_policy_auto_grants_root_writable() {
    let root = Path::new("/repo");
    let policy = worktree_policy(root);
    assert!(policy.writable_roots.iter().any(|p| p == root));
}

#[test]
fn worktree_policy_protects_git_austringer_agents_and_settings() {
    let root = Path::new("/repo");
    let policy = worktree_policy(root);
    for rel in [".git", ".austringer", ".agents", "settings.toml"] {
        let expected = root.join(rel);
        assert!(
            policy.protected_paths.contains(&expected),
            "protected paths must include {expected:?}"
        );
    }
}

#[test]
fn default_tier_is_worktree() {
    let policy = worktree_policy(Path::new("/repo"));
    assert!(matches!(policy.tier, SandboxTier::Worktree));
}

#[test]
fn egress_allowlist_starts_empty() {
    let policy = worktree_policy(Path::new("/repo"));
    assert!(
        policy.egress.is_empty(),
        "Droid AVOID: zero built-in vendor-escape domains"
    );
}

#[test]
fn env_scrub_defaults_drop_key_secret_token() {
    let policy = worktree_policy(Path::new("/repo"));
    assert!(policy.env_scrub.drops("ANTHROPIC_API_KEY"));
    assert!(policy.env_scrub.drops("MY_SECRET"));
    assert!(policy.env_scrub.drops("GITHUB_TOKEN"));
    assert!(!policy.env_scrub.drops("PATH"));
}

#[test]
fn pinned_profile_round_trips_through_serde() {
    let policy = worktree_policy(Path::new("/repo"));
    let pinned = PinnedProfile::pin("session-1", policy);
    let serialized = serde_json::to_string(&pinned).expect("serde");
    let restored = restore(&serialized).expect("restore");
    assert_eq!(restored.session_id, "session-1");
    assert_eq!(restored.policy.tier, SandboxTier::Worktree);
}

#[test]
fn restore_rejects_corrupted_pin_with_typed_error() {
    let result = restore("{not json");
    assert!(result.is_err());
}

#[test]
fn mid_session_profile_change_is_typed_rejection() {
    let policy = worktree_policy(Path::new("/repo"));
    let pinned = PinnedProfile::pin("session-1", policy);
    let mut looser = worktree_policy(Path::new("/repo"));
    looser.writable_roots.push(Path::new("/").to_path_buf());
    match attempt_change(&pinned, &looser) {
        Err(PinError::ProfilePinned { session_id, .. }) => assert_eq!(session_id, "session-1"),
        other => panic!("expected ProfilePinned, got {other:?}"),
    }
}

#[test]
fn merge_denies_win_protected_union_and_writables_intersect() {
    let base_root = Path::new("/base");
    let over_root = Path::new("/over");
    let mut base = worktree_policy(base_root);
    base.writable_roots.push(Path::new("/shared").to_path_buf());
    let mut over = worktree_policy(over_root);
    over.writable_roots.push(Path::new("/shared").to_path_buf());
    over.protected_paths.push(base_root.join("secrets.env"));

    let merged = merge_policies(&base, &over);
    assert!(
        merged
            .protected_paths
            .contains(&base_root.join("secrets.env")),
        "protected paths are a union (deny wins)"
    );
    assert!(
        !merged.writable_roots.contains(&base_root.to_path_buf()),
        "writable roots intersect: base-only root must not survive"
    );
    assert!(
        merged
            .writable_roots
            .contains(&Path::new("/shared").to_path_buf())
    );
    assert!(
        merged.egress.is_empty(),
        "egress allowlists intersect (empty and empty stays empty)"
    );
}

#[test]
fn violation_event_carries_kind_and_decision_seam() {
    let event = ViolationEvent::new(ViolationKind::ProtectedPathWrite, ".git/config");
    assert_eq!(event.kind, ViolationKind::ProtectedPathWrite);
    let decision = ViolationDecision::Deny;
    assert_eq!(event.default_decision(), decision);
}

#[test]
fn escape_gate_requires_once_per_thread_approval() {
    let mut gate = EscapeGate::new();
    assert!(!gate.may_escape_unsandboxed(7));
    gate.approve_thread(7);
    assert!(
        gate.may_escape_unsandboxed(7),
        "second ask: already approved"
    );
    assert!(
        !gate.may_escape_unsandboxed(8),
        "approval is per-thread, does not leak to other threads"
    );
}

#[test]
fn deny_glob_parses_and_exposes_pattern() {
    let glob = DenyGlob::parse(".git/**").expect("parse");
    assert_eq!(glob.pattern(), ".git/**");
}
