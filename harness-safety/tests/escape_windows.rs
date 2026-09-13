//! Windows RUNTIME escape-attempt suite (todo 26 acceptance).
//!
//! Real processes, real kernel enforcement, on this machine:
//! allowed writes inside the worktree root succeed transparently; writes
//! outside the root, writes to protected paths (.git, settings file), and
//! deny-glob targets are DENIED with EPERM-class evidence (nonzero exit +
//! absent file), never a boolean alone; unenforceable isolation refuses
//! to start.

#![cfg(windows)]

use harness_safety::backends::{SandboxCommand, SandboxError, platform_backend};
use harness_safety::policy::decl::{DenyGlob, SandboxPolicy};
use std::path::{Path, PathBuf};

const EXIT_ESCAPE: i32 = 42; // probe wrote the file
const EXIT_DENIED: i32 = 5; // probe hit OS denial

fn probe_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_harness-safety-probe"))
}

struct Fixture {
    #[allow(dead_code)]
    base: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
    probe_in_root: PathBuf,
}

fn fixture() -> Fixture {
    let base = tempfile::tempdir().expect("tempdir");
    let root = base.path().join("worktree");
    let outside = base.path().join("outside");
    let dotgit = root.join(".git");
    std::fs::create_dir_all(&dotgit).expect("git dir");
    std::fs::create_dir_all(&outside).expect("outside dir");
    std::fs::write(dotgit.join("HEAD"), b"ref: refs/heads/main").expect("HEAD");
    std::fs::write(root.join("settings.toml"), b"[sandbox]").expect("settings");
    // the probe exe must live inside a granted root: AppContainer image-open
    // uses the sandboxed token
    let probe_in_root = root.join("probe.exe");
    std::fs::copy(probe_exe(), &probe_in_root).expect("copy probe");
    Fixture {
        base,
        root,
        outside,
        probe_in_root,
    }
}

fn spawn_probe(
    sandbox: &harness_safety::backends::EnforcedSandbox,
    fixture: &Fixture,
    target: &Path,
) -> i32 {
    let command = SandboxCommand {
        program: fixture.probe_in_root.clone(),
        args: vec!["--write".into(), target.to_string_lossy().into_owned()],
        env: vec![("HARNESS_SANDBOX_TEST".into(), "1".into())],
        cwd: fixture.root.clone(),
    };
    let mut child = sandbox.spawn(&command).expect("spawn");
    child.wait().expect("wait")
}

fn policy_for(fixture: &Fixture) -> SandboxPolicy {
    SandboxPolicy::for_worktree(&fixture.root, &fixture.root.join("settings.toml"))
}

#[test]
fn allowed_write_inside_root_is_transparent() {
    let fx = fixture();
    let sandbox = platform_backend()
        .enforce(&policy_for(&fx))
        .expect("enforce");
    let target = fx.root.join("allowed.txt");
    let code = spawn_probe(&sandbox, &fx, &target);
    assert_eq!(code, EXIT_ESCAPE, "allowed op must succeed transparently");
    assert!(
        target.exists(),
        "allowed write must land on disk (not booleans - on-disk evidence)"
    );
}

#[test]
fn write_outside_root_is_denied_by_kernel() {
    let fx = fixture();
    let sandbox = platform_backend()
        .enforce(&policy_for(&fx))
        .expect("enforce");
    let target = fx.outside.join("escape.txt");
    let code = spawn_probe(&sandbox, &fx, &target);
    assert_eq!(
        code, EXIT_DENIED,
        "write outside writable roots must be kernel-denied"
    );
    assert!(!target.exists(), "no on-disk evidence of the escape");
}

#[test]
fn write_to_protected_git_metadata_is_denied() {
    let fx = fixture();
    let sandbox = platform_backend()
        .enforce(&policy_for(&fx))
        .expect("enforce");
    let target = fx.root.join(".git").join("config");
    let code = spawn_probe(&sandbox, &fx, &target);
    assert_eq!(code, EXIT_DENIED, ".git must stay protected");
    assert!(!target.exists());
    assert!(
        fx.root.join(".git").join("HEAD").exists(),
        "pre-existing protected content is intact"
    );
}

#[test]
fn write_to_settings_file_is_denied() {
    let fx = fixture();
    let sandbox = platform_backend()
        .enforce(&policy_for(&fx))
        .expect("enforce");
    let target = fx.root.join("settings.toml");
    let code = spawn_probe(&sandbox, &fx, &target);
    assert_eq!(code, EXIT_DENIED, "sandbox guards its own settings file");
    assert_eq!(
        std::fs::read_to_string(&target).expect("read"),
        "[sandbox]",
        "settings file content untouched"
    );
}

#[test]
fn deny_glob_target_is_denied_when_resolvable() {
    let fx = fixture();
    std::fs::write(fx.root.join("server.pem"), b"LEAST PRIVILEGE").expect("pem");
    let mut policy = policy_for(&fx);
    policy
        .deny_globs
        .push(DenyGlob::parse("*.pem").expect("parse"));
    let sandbox = platform_backend().enforce(&policy).expect("enforce");
    let target = fx.root.join("server.pem");
    let code = spawn_probe(&sandbox, &fx, &target);
    assert_eq!(code, EXIT_DENIED, "deny-glob must be kernel-enforced");
}

#[test]
fn unresolvable_glob_refuses_to_start() {
    let fx = fixture();
    let mut policy = policy_for(&fx);
    // zero .env files exist under the root: nothing to enforce against
    policy
        .deny_globs
        .push(DenyGlob::parse("*.env").expect("parse"));
    match platform_backend().enforce(&policy) {
        Err(SandboxError::RefusedToStart { .. }) => {}
        Err(other) => panic!("expected RefusedToStart, got {other}"),
        Ok(_) => panic!("expected RefusedToStart, got Ok"),
    }
}

#[test]
fn report_shows_enforced_equals_requested_for_successful_enforce() {
    let fx = fixture();
    let sandbox = platform_backend()
        .enforce(&policy_for(&fx))
        .expect("enforce");
    let report = sandbox.report();
    let caps: Vec<&str> = report.gaps.iter().map(|g| g.capability.as_str()).collect();
    assert!(
        !caps.iter().any(|c| c.starts_with("fs_") || *c == "network"),
        "fs and network isolation enforced == requested, gaps: {caps:?}"
    );
    assert_eq!(
        report.enforced.fs_write,
        harness_safety::report::Isolation::Changes
    );
    assert_eq!(
        report.enforced.network,
        harness_safety::report::NetworkIsolation::Kernel
    );
}

#[test]
fn spawned_child_dies_with_job_domain() {
    let fx = fixture();
    let sandbox = platform_backend()
        .enforce(&policy_for(&fx))
        .expect("enforce");
    let command = SandboxCommand {
        program: PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        args: vec![
            "/d".into(),
            "/c".into(),
            "ping -n 30 127.0.0.1 > nul".into(),
        ],
        env: vec![],
        cwd: fx.root.clone(),
    };
    let mut child = sandbox.spawn(&command).expect("spawn");
    let started = std::time::Instant::now();
    child.kill_tree();
    // a job-killed process has no defined exit code; the proof of the
    // kill domain is that a 30-second command is gone in seconds, not
    // that the code is nonzero
    let code = child.wait().expect("wait after kill");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "kill domain must reap the tree promptly, took {:?}",
        started.elapsed()
    );
    let _ = code;
}

#[test]
fn env_scrub_gap_is_reported_honestly_on_this_backend() {
    // This Windows build rejects custom environments on LowBox spawns
    // (os error 203), so the backend cannot scrub at spawn time. The
    // scrub POLICY is real and unit-tested (tests/scrub.rs) and is
    // enforced at the exec-layer command seam; here we prove the report
    // never CLAIMS scrubbing it did not do (honest isolation reporting).
    let fx = fixture();
    let sandbox = platform_backend()
        .enforce(&policy_for(&fx))
        .expect("enforce");
    let report = sandbox.report();
    assert!(!report.enforced.env_scrubbed);
    assert!(
        report.gaps.iter().any(|g| g.capability == "env_scrub"),
        "the env-scrub limitation must be a NAMED gap, got {:?}",
        report.gaps
    );
}
