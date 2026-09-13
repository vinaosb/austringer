//! Container backend arg generation (todo 26) - feature-gated.

#![cfg(feature = "container")]

use harness_safety::backends::container::{ContainerSpec, docker_args};
use harness_safety::policy::decl::SandboxPolicy;
use std::path::Path;

#[test]
fn hardened_container_defaults_always_present() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let args = docker_args(
        &policy,
        &ContainerSpec {
            image: "alpine:3".into(),
        },
    );
    let joined = args.join(" ");
    assert!(
        joined.contains("--cap-drop=ALL"),
        "hermes: cap-drop ALL by default"
    );
    assert!(joined.contains("--security-opt=no-new-privileges"));
    assert!(joined.contains("--pids-limit=512"));
    assert!(
        joined.contains("--read-only"),
        "read-only rootfs by default"
    );
    assert!(
        joined.contains("--network=none"),
        "empty egress allowlist = no network"
    );
}

#[test]
fn writable_roots_mount_as_rw_read_only_elsewhere() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("settings.toml"));
    let args = docker_args(
        &policy,
        &ContainerSpec {
            image: "alpine:3".into(),
        },
    );
    let mount = format!("--volume={}:{}:rw", root.display(), "/workspace/repo");
    assert!(
        args.contains(&mount),
        "worktree root auto-granted writable: {args:?}"
    );
}

#[test]
fn image_and_command_close_the_args() {
    let root = Path::new("/repo");
    let policy = SandboxPolicy::for_worktree(root, &root.join("s.toml"));
    let args = docker_args(
        &policy,
        &ContainerSpec {
            image: "alpine:3".into(),
        },
    );
    assert_eq!(*args.last().expect("image last"), "alpine:3");
}

// One daemon smoke run, manual (`-- --ignored`): the generated args must
// actually execute under the installed Docker engine. Skipped silently
// when no daemon is reachable so CI legs without Docker stay green.
#[test]
#[ignore = "requires a reachable Docker daemon"]
fn docker_daemon_smoke_run() {
    let probe = std::process::Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output()
        .expect("docker binary");
    if !probe.status.success() {
        eprintln!("docker daemon unreachable; skipping smoke");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("worktree")).expect("mkdir");
    let policy = SandboxPolicy::for_worktree(
        &dir.path().join("worktree"),
        &dir.path().join("worktree/settings.toml"),
    );
    let mut args = docker_args(
        &policy,
        &ContainerSpec {
            image: "alpine:3".into(),
        },
    );
    args.push("sh".into());
    args.push("-c".into());
    args.push("echo isolated > /workspace/worktree/proof.txt".into());
    let status = std::process::Command::new("docker")
        .args(&args)
        .status()
        .expect("docker run");
    assert!(
        status.success(),
        "hardened container args must run cleanly under the daemon"
    );
    assert!(
        dir.path().join("worktree/proof.txt").exists(),
        "the writable-root mount must receive the container's write"
    );
}
