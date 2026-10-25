//! Windows sandbox backend (plan todo 26): AppContainer enforcement for
//! standard users, real and runtime-tested on this machine.
//!
//! Mechanism (kernel-enforced, no admin privileges required):
//!
//! 1. **AppContainer profile** — a per-enforcement package SID; the
//!    child process is launched with the LowBox token Windows builds
//!    from `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES`. The
//!    restricted-token `CreateProcessAsUser` route was probed and
//!    rejected: error 1314 for every variant (see task-26 evidence).
//! 2. **ACLs** — grant ACEs for the package SID on writable roots
//!    (readable roots get read-only grants), deny-write ACEs on
//!    protected paths and resolved deny-globs. AppContainer access
//!    checks are dual-principal: objects without an explicit package-SID
//!    ACE are deny-by-default, which IS the outside-root confinement.
//! 3. **Job Object** — `KILL_ON_JOB_CLOSE` tree containment, assigned
//!    immediately after spawn (thin re-implementation of the todo-19
//!    pattern; `austringer-exec`'s `TreeGuard` takes a PTY child and
//!    safety must not depend on exec — the dependency direction is
//!    exec → safety).
//!
//! Network: an AppContainer with zero capabilities has no network
//! capability, so the WFP layer denies outbound by default (kernel
//! deny-by-default); allowlisted egress flows through the userland
//! filter proxy (see [`crate::egress`]).

use std::sync::Arc;

use crate::backends::win_ffi::{self, AcProfile};
use crate::backends::{
    EnforcedSandbox, RefusalReason, SandboxBackend, SandboxCommand, SandboxError, SandboxedChild,
    SandboxedChildHandle, SpawnInSandbox,
};
use crate::policy::decl::SandboxPolicy;
use crate::report::{EnforcedSummary, Isolation, IsolationReport, NetworkIsolation};

/// The Windows backend.
pub struct WindowsSandbox;

/// Static backend instance.
pub static WINDOWS_SANDBOX: WindowsSandbox = WindowsSandbox;

struct Enforcement {
    sid_sddl: Arc<String>,
    profile: Arc<AcProfile>,
}

impl Drop for Enforcement {
    fn drop(&mut self) {
        self.profile.delete();
    }
}

impl SpawnInSandbox for Enforcement {
    fn spawn(&self, command: &SandboxCommand) -> Result<SandboxedChild, SandboxError> {
        let spawned = crate::backends::win_spawn::spawn_in_appcontainer(
            &self.sid_sddl,
            &command.program.to_string_lossy(),
            &command.args,
            &command.cwd,
        )
        .map_err(|e| SandboxError::SpawnFailed {
            detail: e.to_string(),
        })?;
        let job = win32job::Job::create().map_err(|e| SandboxError::SpawnFailed {
            detail: e.to_string(),
        })?;
        let mut limits =
            job.query_extended_limit_info()
                .map_err(|e| SandboxError::SpawnFailed {
                    detail: e.to_string(),
                })?;
        limits.limit_kill_on_job_close();
        job.set_extended_limit_info(&limits)
            .map_err(|e| SandboxError::SpawnFailed {
                detail: e.to_string(),
            })?;
        job.assign_process(spawned.process as isize).map_err(|e| {
            spawned.close();
            SandboxError::SpawnFailed {
                detail: e.to_string(),
            }
        })?;
        Ok(SandboxedChild {
            inner: Box::new(WindowsChild {
                spawned: Some(spawned),
                job: Some(job),
            }),
        })
    }
}

struct WindowsChild {
    spawned: Option<crate::backends::win_spawn::AcSpawned>,
    job: Option<win32job::Job>,
}

impl SandboxedChildHandle for WindowsChild {
    fn wait(&mut self) -> Result<i32, SandboxError> {
        let Some(spawned) = self.spawned.take() else {
            return Err(SandboxError::WaitFailed {
                detail: "already waited".into(),
            });
        };
        // after kill_tree the job is gone; the kernel still signals the
        // process handle, so wait_exit works for both natural exits and
        // job kills
        let code = spawned.wait_exit().map_err(|e| SandboxError::WaitFailed {
            detail: e.to_string(),
        });
        spawned.close();
        code
    }

    fn kill_tree(&mut self) {
        // dropping the last job handle is the kernel kill signal; the
        // process handle stays open so a later wait() observes the death
        self.job = None;
    }
}

impl SandboxBackend for WindowsSandbox {
    fn name(&self) -> &'static str {
        "windows-appcontainer"
    }

    fn enforce(&self, policy: &SandboxPolicy) -> Result<EnforcedSandbox, SandboxError> {
        crate::backends::validate_or_refuse(policy)?;
        let profile =
            AcProfile::create(&profile_name()).map_err(|e| SandboxError::RefusedToStart {
                reason: RefusalReason::IsolationUnenforceable {
                    detail: format!("AppContainer profile unavailable: {e}"),
                },
            })?;
        let enforcement = apply_policy_aces(policy, &profile)?;
        // env_scrubbed is FALSE here and the report names the gap: this
        // build rejects custom environments on LowBox spawns (os error
        // 203, see win_ffi), so scrubbing is enforced at the exec-layer
        // command seam instead — never claimed where it did not happen.
        let enforced = EnforcedSummary {
            fs_write: Isolation::Changes,
            fs_read: Isolation::Changes,
            network: NetworkIsolation::Kernel,
            env_scrubbed: false,
        };
        let report = IsolationReport::diff(policy, enforced);
        Ok(EnforcedSandbox::new(
            report,
            Box::new(Enforcement {
                sid_sddl: Arc::new(enforcement),
                profile: Arc::new(profile),
            }),
        ))
    }
}

/// Apply every grant/deny the policy implies; any failure refuses the
/// whole enforcement (fail-closed) and tears the profile down.
///
/// ORDER IS SECURITY-CRITICAL: protected paths are sealed (deny ACE +
/// PROTECTED DACL) BEFORE writable roots are granted. Protection marks
/// the DACL non-inheriting, so the root's later grant propagation
/// skips them — the package SID never receives a grant on a protected
/// path, which (not the deny ACE itself) is what enforces the block:
/// AppContainer's dual-principal check needs a grant and ignores
/// package-SID deny ACEs.
fn apply_policy_aces(policy: &SandboxPolicy, profile: &AcProfile) -> Result<String, SandboxError> {
    let sid = profile.sid_sddl.clone();
    let deny = |path: &std::path::Path| {
        // a protected path that does not exist yet is MATERIALIZED as a
        // directory before the deny ACE: otherwise the child could
        // create `.austringer` itself and own an unprotected write zone
        // inside the root (name-based DACL denial does not exist)
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|e| SandboxError::RefusedToStart {
                reason: RefusalReason::IsolationUnenforceable {
                    detail: format!("protected path {} unmaterializable: {e}", path.display()),
                },
            })?;
        }
        apply(&sid, path, win_ffi::DENY_WRITE, true)
    };
    for path in &policy.protected_paths {
        deny(path)?;
    }
    for glob in &policy.deny_globs {
        for root in &policy.writable_roots {
            let Ok(hits) = glob.resolve(root) else {
                continue;
            };
            for hit in hits {
                deny(&hit)?;
            }
        }
    }
    for root in &policy.writable_roots {
        if !root.exists() {
            std::fs::create_dir_all(root).map_err(|e| SandboxError::RefusedToStart {
                reason: RefusalReason::IsolationUnenforceable {
                    detail: format!("writable root {} unmaterializable: {e}", root.display()),
                },
            })?;
        }
        apply(&sid, root, win_ffi::GRANT_WRITABLE, false)?;
    }
    for root in &policy.readable_roots {
        if root.exists() {
            apply(&sid, root, win_ffi::GRANT_READABLE, false)?;
        }
    }
    Ok(sid)
}

fn apply(sid: &str, path: &std::path::Path, mask: u32, is_deny: bool) -> Result<(), SandboxError> {
    win_ffi::apply_ace(sid, path, mask, is_deny).map_err(|e| SandboxError::RefusedToStart {
        reason: RefusalReason::IsolationUnenforceable {
            detail: format!("ACL enforcement failed on {}: {e}", path.display()),
        },
    })
}

fn profile_name() -> String {
    let mut bytes = [0u8; 6];
    if getrandom::fill(&mut bytes).is_err() {
        return "austringer-safety-0".to_string();
    }
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("austringer-safety-{hex}")
}
