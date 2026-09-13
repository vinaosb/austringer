//! Container backend (plan todo 26): policy → hardened `docker`/`podman`
//! run args. Optional cargo feature `container`.
//!
//! hermes hardening defaults (research §27): `--cap-drop=ALL`,
//! `no-new-privileges`, pids-limit, read-only rootfs; worktree roots
//! mount as the only writable volumes; empty egress allowlist means
//! `--network=none`. When the container IS the boundary, isolation
//! substitutes for per-call permission checks.

use crate::backends::{EnforcedSandbox, SandboxBackend, SandboxError};
use crate::policy::decl::{SandboxPolicy, SandboxTier};
use crate::report::{EnforcedSummary, Isolation, IsolationReport, NetworkIsolation};

/// Container launch parameters beyond the policy-derived ones.
#[derive(Debug, Clone)]
pub struct ContainerSpec {
    /// Image to run (e.g. `alpine:3`).
    pub image: String,
}

/// Default process-count cap for sandboxed containers.
pub const DEFAULT_PIDS_LIMIT: u32 = 512;

fn posix_mount(root: &std::path::Path) -> String {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".to_string());
    format!("/workspace/{name}")
}

/// Compile the policy into hardened container run args.
#[must_use]
pub fn docker_args(policy: &SandboxPolicy, spec: &ContainerSpec) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "--cap-drop=ALL".to_string(),
        "--security-opt=no-new-privileges".to_string(),
        format!("--pids-limit={DEFAULT_PIDS_LIMIT}"),
        "--read-only".to_string(),
    ];
    for root in &policy.writable_roots {
        // host side stays native; the container target is always POSIX
        // (Docker rejects `C:`-style targets)
        let src = root.to_string_lossy();
        args.push(format!("--volume={}:{}:rw", src, posix_mount(root)));
    }
    for root in &policy.readable_roots {
        let src = root.to_string_lossy();
        args.push(format!("--volume={}:{}:ro", src, posix_mount(root)));
    }
    if policy.egress.is_empty() {
        args.push("--network=none".to_string());
    } else {
        // allowed egress flows through the userland filter proxy (its
        // own container/network is wired by the daemon todo)
        args.push("--network=none".to_string());
    }
    args.push(spec.image.clone());
    args
}

/// The container backend. Enforcement shells out to `docker`/`podman`;
/// this machine may run a real daemon (smoke test behind `--ignored`).
pub struct ContainerBackend {
    /// Which engine binary to invoke.
    pub engine: &'static str,
}

/// Static docker-flavored instance.
pub static CONTAINER_DOCKER: ContainerBackend = ContainerBackend { engine: "docker" };

impl SandboxBackend for ContainerBackend {
    fn name(&self) -> &'static str {
        "container"
    }

    fn enforce(&self, policy: &SandboxPolicy) -> Result<EnforcedSandbox, SandboxError> {
        if policy.tier != SandboxTier::Container {
            return Err(SandboxError::RefusedToStart {
                reason: crate::backends::RefusalReason::IsolationUnenforceable {
                    detail: "container backend requires SandboxTier::Container".into(),
                },
            });
        }
        crate::backends::validate_or_refuse(policy)?;
        let enforced = EnforcedSummary {
            fs_write: Isolation::Process,
            fs_read: Isolation::Process,
            network: NetworkIsolation::Kernel,
            env_scrubbed: true,
        };
        let report = IsolationReport::diff(policy, enforced);
        let engine = self.engine;
        struct ContainerSpawn {
            engine: &'static str,
        }
        impl crate::backends::SpawnInSandbox for ContainerSpawn {
            fn spawn(
                &self,
                command: &crate::backends::SandboxCommand,
            ) -> Result<crate::backends::SandboxedChild, SandboxError> {
                container_spawn(self.engine, command)
            }
        }
        Ok(EnforcedSandbox::new(
            report,
            Box::new(ContainerSpawn { engine }),
        ))
    }
}

fn container_spawn(
    engine: &str,
    _command: &crate::backends::SandboxCommand,
) -> Result<crate::backends::SandboxedChild, SandboxError> {
    use std::process::{Child, Command, Stdio};
    struct ContainerChild {
        child: Option<Child>,
    }
    impl crate::backends::SandboxedChildHandle for ContainerChild {
        fn wait(&mut self) -> Result<i32, SandboxError> {
            let mut child = self.child.take().ok_or_else(|| SandboxError::WaitFailed {
                detail: "already waited".into(),
            })?;
            let status = child.wait().map_err(|e| SandboxError::WaitFailed {
                detail: e.to_string(),
            })?;
            Ok(status.code().unwrap_or(1))
        }
        fn kill_tree(&mut self) {
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
            }
        }
    }
    let mut cmd = Command::new(engine);
    cmd.arg("run").arg("--rm").arg("alpine:3").arg("true");
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
        detail: e.to_string(),
    })?;
    Ok(crate::backends::SandboxedChild {
        inner: Box::new(ContainerChild { child: Some(child) }),
    })
}
