//! Deny-glob fail-closed tests (todo 26): an unresolvable glob must REFUSE
//! enforcement, never allow-by-failure.

#![cfg(test)]

use austringer_safety::policy::decl::{DenyGlob, SandboxPolicy};
use std::path::Path;

#[test]
fn malformed_glob_is_typed_parse_error() {
    for bad in ["[unclosed", "a**b[", "]", "\\"] {
        assert!(
            DenyGlob::parse(bad).is_err(),
            "pattern {bad:?} must not parse"
        );
    }
}

#[test]
fn glob_matching_honors_star_question_and_class() {
    let glob = DenyGlob::parse("*.pem").expect("parse");
    assert!(glob.matches_text("server.pem"));
    assert!(!glob.matches_text("server.pem.bak"));

    let q = DenyGlob::parse("secret?").expect("parse");
    assert!(q.matches_text("secret1"));
    assert!(!q.matches_text("secret12"));

    let class = DenyGlob::parse("key[0-9].txt").expect("parse");
    assert!(class.matches_text("key7.txt"));
    assert!(!class.matches_text("keyx.txt"));
}

#[test]
fn recursive_glob_matches_nested_paths() {
    let glob = DenyGlob::parse(".git/**").expect("parse");
    assert!(glob.matches_text(".git/config"));
    assert!(glob.matches_text(".git/objects/ab/cdef"));
    assert!(!glob.matches_text(".gitignore"));
}

#[test]
fn resolve_finds_existing_matches_under_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let git = dir.path().join(".git");
    std::fs::create_dir_all(git.join("objects")).expect("mkdir");
    std::fs::write(git.join("config"), b"[core]").expect("write");
    let glob = DenyGlob::parse(".git/**").expect("parse");
    let resolved = glob
        .resolve(dir.path())
        .expect("resolvable: matches exist on disk");
    assert!(resolved.len() >= 3, "config + objects/ + .git itself");
}

#[test]
fn resolve_with_zero_disk_matches_is_unresolvable() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("readme.md"), b"x").expect("write");
    let glob = DenyGlob::parse("*.pem").expect("parse");
    let error = glob
        .resolve(dir.path())
        .expect_err("no .pem exists: nothing to kernel-enforce");
    assert!(
        error.is_unresolvable(),
        "zero-match glob must be Unresolvable, got {error:?}"
    );
}

#[test]
fn policy_with_zero_match_glob_is_refused_at_enforce_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".git")).expect("mkdir");
    let mut policy = SandboxPolicy::for_worktree(dir.path(), &dir.path().join("settings.toml"));
    policy
        .deny_globs
        .push(DenyGlob::parse("*.pem").expect("parse"));
    let validation = policy.validate();
    assert!(
        validation.is_err(),
        "policy validation must fail closed on unresolvable globs"
    );
    let message = validation.expect_err("err").to_string();
    assert!(
        message.to_lowercase().contains("unresolvable"),
        "error names the refusal reason: {message}"
    );
}

#[test]
fn policy_validation_rejects_relative_roots() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut policy = SandboxPolicy::for_worktree(dir.path(), &dir.path().join("s.toml"));
    policy
        .writable_roots
        .push(Path::new("relative/root").to_path_buf());
    assert!(policy.validate().is_err());
}
