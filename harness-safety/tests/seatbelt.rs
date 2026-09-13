//! Seatbelt profile generation tests (todo 26) - cross-platform: the
//! PROFILE STRING is testable everywhere; enforcement is macOS-only.

#![cfg(test)]

use harness_safety::backends::macos_sandbox::SeatbeltProfile;
use harness_safety::policy::decl::{DenyGlob, SandboxPolicy};
use std::path::Path;

#[test]
fn writable_roots_become_file_write_subpath_allows() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let profile = SeatbeltProfile::from_policy(&policy).render();
    assert!(profile.contains("(allow file-write* (subpath \"/repo\"))"));
}

#[test]
fn protected_paths_become_file_write_denies() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let profile = SeatbeltProfile::from_policy(&policy).render();
    assert!(profile.contains("(deny file-write* (subpath \"/repo/.git\"))"));
    assert!(profile.contains("(deny file-write* (subpath \"/repo/settings.toml\"))"));
}

#[test]
fn deny_globs_render_as_literal_subpath_denies() {
    let root = Path::new("/repo");
    let mut policy = SandboxPolicy::for_worktree(root, &root.join("s.toml"));
    policy
        .deny_globs
        .push(DenyGlob::parse(".harness/**").expect("parse"));
    let profile = SeatbeltProfile::from_policy(&policy).render();
    assert!(profile.contains("(deny file-write* (subpath \"/repo/.harness\"))"));
}

#[test]
fn empty_egress_renders_network_deny_all() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("s.toml"));
    let profile = SeatbeltProfile::from_policy(&policy).render();
    assert!(
        profile.contains("(deny network*)"),
        "empty allowlist must render as blanket network deny"
    );
}

#[test]
fn allowed_hosts_render_scoped_network_outbound() {
    let root = Path::new("/repo");
    let mut policy = SandboxPolicy::for_worktree(root, &root.join("s.toml"));
    policy
        .egress
        .allow(harness_safety::egress::HostPattern::Exact(
            "api.example.com".into(),
        ))
        .expect("allow");
    let profile = SeatbeltProfile::from_policy(&policy).render();
    assert!(profile.contains("(allow network-outbound (remote ip \"api.example.com:443\"))"));
    assert!(!profile.contains("(deny network*)"));
}

#[test]
fn profile_string_ends_with_version_header() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("s.toml"));
    let profile = SeatbeltProfile::from_policy(&policy).render();
    assert!(profile.starts_with(";; harness-safety seatbelt profile v1"));
}
