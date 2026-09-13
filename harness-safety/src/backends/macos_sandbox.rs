//! macOS Seatbelt backend (plan todo 26).
//!
//! The PROFILE STRING generation is pure and tested cross-platform
//! (see `tests/seatbelt.rs`); `enforce` runs `sandbox-exec -p` and is
//! compiled only on macOS — runtime proof is owned by the todo-3 CI
//! macOS leg.

use std::path::Path;

#[cfg(target_os = "macos")]
use crate::backends::SandboxedChildHandle;
#[cfg(target_os = "macos")]
use crate::backends::{
    EnforcedSandbox, SandboxBackend, SandboxError, SandboxedChild, SpawnInSandbox,
};
#[cfg(not(target_os = "macos"))]
use crate::backends::{
    EnforcedSandbox, SandboxBackend, SandboxError, SandboxedChild, SpawnInSandbox,
};
use crate::policy::decl::SandboxPolicy;
use crate::report::{EnforcedSummary, Isolation, IsolationReport, NetworkIsolation};

/// Renders a `sandbox-exec` profile from one typed policy.
#[derive(Debug, Clone)]
pub struct SeatbeltProfile {
    lines: Vec<String>,
}

impl SeatbeltProfile {
    /// Compile the policy into Seatbelt directives.
    #[must_use]
    pub fn from_policy(policy: &SandboxPolicy) -> Self {
        let mut lines = vec![";; harness-safety seatbelt profile v1".to_string()];
        for root in &policy.writable_roots {
            lines.push(format!("(allow file-write* (subpath \"{}\"))", norm(root)));
        }
        for root in &policy.readable_roots {
            lines.push(format!("(allow file-read* (subpath \"{}\"))", norm(root)));
        }
        // deny-after-allow: protected paths override writable roots
        for path in &policy.protected_paths {
            lines.push(format!("(deny file-write* (subpath \"{}\"))", norm(path)));
        }
        for glob in &policy.deny_globs {
            for hit in resolve_glob_paths(policy, glob.pattern()) {
                lines.push(format!("(deny file-write* (subpath \"{}\"))", norm(&hit)));
            }
        }
        if policy.egress.is_empty() {
            lines.push("(deny network*)".to_string());
        } else {
            for pattern in policy.egress.iter() {
                let rendered = pattern.to_string();
                let remote = if rendered.contains(':') {
                    rendered
                } else {
                    format!("{rendered}:443")
                };
                lines.push(format!("(allow network-outbound (remote ip \"{remote}\"))"));
            }
        }
        Self { lines }
    }

    /// The full profile text for `sandbox-exec -p`.
    #[must_use]
    pub fn render(&self) -> String {
        self.lines.join("\n")
    }
}

fn resolve_glob_paths(policy: &SandboxPolicy, pattern: &str) -> Vec<std::path::PathBuf> {
    policy
        .writable_roots
        .iter()
        .filter_map(|root| {
            let glob = crate::policy::glob::DenyGlob::parse(pattern).ok()?;
            glob.resolve(root).ok()
        })
        .flatten()
        .collect()
}

fn norm(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The macOS backend (enforcement macOS-only).
pub struct MacosSandbox;

/// Static backend instance.
pub static MACOS_SANDBOX: MacosSandbox = MacosSandbox;

impl SandboxBackend for MacosSandbox {
    fn name(&self) -> &'static str {
        "macos-seatbelt"
    }

    fn enforce(&self, policy: &SandboxPolicy) -> Result<EnforcedSandbox, SandboxError> {
        crate::backends::validate_or_refuse(policy)?;
        let profile = SeatbeltProfile::from_policy(policy).render();
        let enforced = EnforcedSummary {
            fs_write: Isolation::Changes,
            fs_read: Isolation::Changes,
            network: NetworkIsolation::Kernel,
            env_scrubbed: true,
        };
        let report = IsolationReport::diff(policy, enforced);
        let holder = profile_box(profile);
        Ok(EnforcedSandbox::new(report, holder))
    }
}

fn profile_box(profile: String) -> Box<dyn SpawnInSandbox> {
    struct SeatbeltSpawn {
        profile: String,
    }
    impl SpawnInSandbox for SeatbeltSpawn {
        fn spawn(
            &self,
            command: &crate::backends::SandboxCommand,
        ) -> Result<SandboxedChild, SandboxError> {
            seatbelt_spawn(&self.profile, command)
        }
    }
    Box::new(SeatbeltSpawn { profile })
}

#[cfg(target_os = "macos")]
fn seatbelt_spawn(
    profile: &str,
    command: &crate::backends::SandboxCommand,
) -> Result<SandboxedChild, SandboxError> {
    use std::process::{Child, Command, Stdio};
    struct SeatbeltChild {
        child: Option<Child>,
    }
    impl SandboxedChildHandle for SeatbeltChild {
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
    let mut cmd = Command::new("/usr/bin/sandbox-exec");
    cmd.arg("-p")
        .arg(profile)
        .arg(&command.program)
        .args(&command.args);
    cmd.current_dir(&command.cwd);
    for (k, v) in &command.env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
        detail: e.to_string(),
    })?;
    Ok(SandboxedChild {
        inner: Box::new(SeatbeltChild { child: Some(child) }),
    })
}

#[cfg(not(target_os = "macos"))]
fn seatbelt_spawn(
    _profile: &str,
    _command: &crate::backends::SandboxCommand,
) -> Result<SandboxedChild, SandboxError> {
    Err(SandboxError::RefusedToStart {
        reason: crate::backends::RefusalReason::IsolationUnenforceable {
            detail: "seatbelt enforcement exists only on macOS".into(),
        },
    })
}
