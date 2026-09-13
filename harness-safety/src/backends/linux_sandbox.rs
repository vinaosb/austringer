//! Linux sandbox backend (plan todo 26): Landlock filesystem rulesets
//! plus a seccomp filter that blocks AF_INET/AF_INET6 sockets when the
//! egress allowlist is empty.
//!
//! **Landlock chosen over bubblewrap** (the plan's alternative):
//! kernel-native, no setuid helper binary, no external dependency on
//! distro packaging; rules map 1:1 onto the typed policy (writable
//! roots get write access-beneath rules, everything else stays
//! read-only). Runtime capability is DETECTED: kernels without Landlock
//! (pre-5.13) or with it disabled produce `ENOSYS`/`EOPNOTSUPP` from
//! `landlock_create_ruleset`, and the backend then REFUSES to start
//! (fail-closed posture) instead of silently running unsandboxed.
//!
//! Cross-compiling proof on this Windows machine: the whole crate is
//! pure Rust, so `cargo clippy -p harness-safety --target
//! x86_64-unknown-linux-gnu` type-checks this module without a Linux
//! linker; runtime proof is owned by the todo-3 CI Linux leg.

use std::os::unix::io::AsRawFd;
use std::path::Path;

use crate::backends::linux_seccomp::install_seccomp_net_filter;
use crate::backends::{
    EnforcedSandbox, RefusalReason, SandboxBackend, SandboxCommand, SandboxError, SandboxedChild,
    SandboxedChildHandle, SpawnInSandbox,
};
use crate::policy::decl::SandboxPolicy;
use crate::report::{EnforcedSummary, Isolation, IsolationReport, NetworkIsolation};
use crate::scrub::scrub_env;

// landlock syscall numbers (x86_64/aarch64 share these)
const SYS_LANDLOCK_CREATE_RULESET: libc::c_long = 444;
const SYS_LANDLOCK_ADD_RULE: libc::c_long = 445;
const SYS_LANDLOCK_RESTRICT_SELF: libc::c_long = 446;

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1 << 0;
const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

// LANDLOCK_ACCESS_FS_* bits (ABI v1-v3 union)
const ACCESS_EXECUTE: u64 = 1 << 0;
const ACCESS_WRITE_FILE: u64 = 1 << 1;
const ACCESS_READ_FILE: u64 = 1 << 2;
const ACCESS_READ_DIR: u64 = 1 << 3;
const ACCESS_REMOVE_DIR: u64 = 1 << 4;
const ACCESS_REMOVE_FILE: u64 = 1 << 5;
const ACCESS_MAKE_REG: u64 = 1 << 8;
const ACCESS_REFER: u64 = 1 << 13;
const ACCESS_TRUNCATE: u64 = 1 << 14;
const HANDLED_ALL: u64 = ACCESS_EXECUTE
    | ACCESS_WRITE_FILE
    | ACCESS_READ_FILE
    | ACCESS_READ_DIR
    | ACCESS_REMOVE_DIR
    | ACCESS_REMOVE_FILE
    | ACCESS_MAKE_REG
    | ACCESS_REFER
    | ACCESS_TRUNCATE;

#[repr(C)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
}

#[repr(C)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
    reserved: i32,
}

/// The Linux backend.
pub struct LinuxSandbox;

/// Static backend instance.
pub static LINUX_SANDBOX: LinuxSandbox = LinuxSandbox;

impl SandboxBackend for LinuxSandbox {
    fn name(&self) -> &'static str {
        "linux-landlock"
    }

    fn enforce(&self, policy: &SandboxPolicy) -> Result<EnforcedSandbox, SandboxError> {
        crate::backends::validate_or_refuse(policy)?;
        if landlock_abi_version().is_none() {
            return Err(SandboxError::RefusedToStart {
                reason: RefusalReason::IsolationUnenforceable {
                    detail: "kernel Landlock unavailable (ENOSYS/EOPNOTSUPP)".into(),
                },
            });
        }
        // bubblewrap detection is not implemented: Landlock is the only
        // supported route, and its absence is a refusal, not a fallback
        let enforced = EnforcedSummary {
            fs_write: Isolation::Changes,
            fs_read: Isolation::Changes,
            network: if policy.egress.is_empty() {
                NetworkIsolation::Kernel
            } else {
                NetworkIsolation::UserlandProxy
            },
            env_scrubbed: true,
        };
        let report = IsolationReport::diff(policy, enforced);
        let roots = policy.writable_roots.clone();
        let seccomp_net = policy.egress.is_empty();
        Ok(EnforcedSandbox::new(
            report,
            Box::new(LandlockSpawn { roots, seccomp_net }),
        ))
    }
}

struct LandlockSpawn {
    roots: Vec<std::path::PathBuf>,
    seccomp_net: bool,
}

impl SpawnInSandbox for LandlockSpawn {
    fn spawn(&self, command: &SandboxCommand) -> Result<SandboxedChild, SandboxError> {
        use std::os::unix::process::CommandExt;
        let roots = self.roots.clone();
        let seccomp_net = self.seccomp_net;
        let mut cmd = std::process::Command::new(&command.program);
        cmd.args(&command.args).current_dir(&command.cwd);
        let pairs = child_env(command);
        cmd.env_clear();
        for (k, v) in &pairs {
            cmd.env(k, v);
        }
        // SAFETY: the pre-exec hook uses only async-signal-safe raw
        // syscalls (openat + landlock + prctl + close).
        unsafe {
            cmd.pre_exec(move || {
                apply_landlock(&roots)?;
                if seccomp_net {
                    install_seccomp_net_filter()?;
                }
                Ok(())
            });
        }
        let child = cmd.spawn().map_err(|e| SandboxError::SpawnFailed {
            detail: e.to_string(),
        })?;
        Ok(SandboxedChild {
            inner: Box::new(UnixChild { child: Some(child) }),
        })
    }
}

struct UnixChild {
    child: Option<std::process::Child>,
}

impl SandboxedChildHandle for UnixChild {
    fn wait(&mut self) -> Result<i32, SandboxError> {
        let Some(mut child) = self.child.take() else {
            return Err(SandboxError::WaitFailed {
                detail: "already waited".into(),
            });
        };
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

fn landlock_abi_version() -> Option<u32> {
    // SAFETY: pure version probe; no out-params.
    let rc = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            core::ptr::null::<LandlockRulesetAttr>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if rc < 0 {
        return None;
    }
    Some(rc as u32)
}

fn apply_landlock(roots: &[std::path::PathBuf]) -> std::io::Result<()> {
    let attr = LandlockRulesetAttr {
        handled_access_fs: HANDLED_ALL,
    };
    // SAFETY: attr sized correctly; classic ruleset creation.
    let fd = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            &attr,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    for root in roots {
        grant_write_beneath(fd as i32, root)?;
    }
    // no_new_privs is required before restrict_self
    // SAFETY: constant prctl arguments.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: fd from create_ruleset above.
    if unsafe { libc::syscall(SYS_LANDLOCK_RESTRICT_SELF, fd as i32, 0) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn grant_write_beneath(ruleset_fd: i32, root: &Path) -> std::io::Result<()> {
    let dir = std::fs::File::open(root)?;
    let rule = LandlockPathBeneathAttr {
        allowed_access: HANDLED_ALL,
        parent_fd: dir.as_raw_fd(),
        reserved: 0,
    };
    // SAFETY: rule references the open dir fd for the duration of the call.
    let rc = unsafe {
        libc::syscall(
            SYS_LANDLOCK_ADD_RULE,
            ruleset_fd,
            LANDLOCK_RULE_PATH_BENEATH,
            &rule,
            0,
        )
    };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn child_env(command: &SandboxCommand) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| crate::scrub::CORE_VARS.contains(&k.as_str()))
        .collect();
    pairs.extend(command.env.iter().cloned());
    scrub_env(&pairs, &crate::scrub::EnvScrub::default()).kept
}
