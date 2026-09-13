//! Sandbox backends (plan todo 26): one policy, many OS-native
//! enforcement layers (Copilot MXC pattern).
//!
//! Fail-closed posture (Droid): when isolation cannot be established the
//! backend returns [`SandboxError::RefusedToStart`] — never a silent
//! degrade, never a green checkmark over an unenforced policy.

use std::path::PathBuf;

use crate::policy::decl::{PolicyError, SandboxPolicy};
use crate::report::IsolationReport;

/// A command to run inside an enforced sandbox.
#[derive(Debug, Clone)]
pub struct SandboxCommand {
    /// Program to execute (absolute path; must be inside a granted root
    /// for token-based backends such as AppContainer).
    pub program: PathBuf,
    /// argv for the program.
    pub args: Vec<String>,
    /// Environment overrides; the policy's scrub applies on top.
    pub env: Vec<(String, String)>,
    /// Working directory (must be inside a writable root).
    pub cwd: PathBuf,
}

/// A live sandboxed child process.
pub struct SandboxedChild {
    /// Platform handle (process/job lifetime owned by the backend).
    pub inner: Box<dyn SandboxedChildHandle>,
}

impl SandboxedChild {
    /// Block until the child exits; returns its exit code.
    ///
    /// # Errors
    /// [`SandboxError::WaitFailed`] when the OS wait surface fails.
    pub fn wait(&mut self) -> Result<i32, SandboxError> {
        self.inner.wait()
    }

    /// Kill the whole sandboxed tree (job object / process group).
    pub fn kill_tree(&mut self) {
        self.inner.kill_tree();
    }
}

/// Platform handle seam implemented by each backend.
pub trait SandboxedChildHandle: Send {
    /// See [`SandboxedChild::wait`].
    ///
    /// # Errors
    /// [`SandboxError::WaitFailed`].
    fn wait(&mut self) -> Result<i32, SandboxError>;
    /// See [`SandboxedChild::kill_tree`].
    fn kill_tree(&mut self);
}

/// An enforced sandbox: the policy compiled into live OS enforcement,
/// plus the honest report of what was actually established.
pub struct EnforcedSandbox {
    report: IsolationReport,
    spawn: Box<dyn SpawnInSandbox>,
}

impl EnforcedSandbox {
    /// (backend constructor use) wrap enforcement state.
    pub(crate) fn new(report: IsolationReport, spawn: Box<dyn SpawnInSandbox>) -> Self {
        Self { report, spawn }
    }

    /// The requested-vs-enforced isolation report.
    #[must_use]
    pub fn report(&self) -> &IsolationReport {
        &self.report
    }

    /// Spawn one command inside the sandbox.
    ///
    /// # Errors
    /// [`SandboxError::SpawnFailed`] when the OS refuses the spawn.
    pub fn spawn(&self, command: &SandboxCommand) -> Result<SandboxedChild, SandboxError> {
        self.spawn.spawn(command)
    }
}

/// Backend seam for spawning inside a live enforcement.
pub trait SpawnInSandbox: Sync + Send {
    /// Spawn `command` under enforcement.
    ///
    /// # Errors
    /// [`SandboxError::SpawnFailed`].
    fn spawn(&self, command: &SandboxCommand) -> Result<SandboxedChild, SandboxError>;
}

/// One OS-native enforcement layer.
pub trait SandboxBackend: Sync {
    /// Backend name for reports and audits.
    fn name(&self) -> &'static str;

    /// Enforce `policy` or refuse to start.
    ///
    /// # Errors
    /// [`SandboxError::RefusedToStart`] when isolation cannot be
    /// established (fail-closed), [`SandboxError::Policy`] when the
    /// policy itself fails validation.
    fn enforce(&self, policy: &SandboxPolicy) -> Result<EnforcedSandbox, SandboxError>;
}

/// Typed sandbox failures. There is deliberately no "degraded" variant.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    /// Whole-process mode refused to start (fail-closed posture).
    #[error("refused to start: {reason}")]
    RefusedToStart {
        /// Machine-readable refusal reason.
        reason: RefusalReason,
    },
    /// The policy failed validation (unresolvable globs, bad roots).
    #[error("policy rejected: {0}")]
    Policy(#[from] PolicyError),
    /// Spawning inside the sandbox failed.
    #[error("spawn failed: {detail}")]
    SpawnFailed {
        /// What the OS refused.
        detail: String,
    },
    /// Waiting on the child failed.
    #[error("wait failed: {detail}")]
    WaitFailed {
        /// What the OS wait refused.
        detail: String,
    },
}

/// Machine-readable refusal taxonomy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RefusalReason {
    /// A deny-glob resolved to nothing enforceable.
    #[error("unresolvable deny-glob {pattern:?}: refusing rather than allow-by-failure")]
    UnresolvableGlob {
        /// The offending pattern.
        pattern: String,
    },
    /// The backend cannot establish the requested isolation at all.
    #[error("isolation unenforceable on this platform/backend: {detail}")]
    IsolationUnenforceable {
        /// What is missing.
        detail: String,
    },
}

/// Fail-closed validation shared by every backend: an unresolvable
/// deny-glob is a refusal to START (`RefusedToStart`), matching the
/// task's "never allow-by-failure" clause, while other policy defects
/// surface as [`SandboxError::Policy`].
pub(crate) fn validate_or_refuse(policy: &SandboxPolicy) -> Result<(), SandboxError> {
    match policy.validate() {
        Ok(()) => Ok(()),
        Err(PolicyError::Glob(crate::policy::glob::GlobError::Unresolvable { pattern })) => {
            Err(SandboxError::RefusedToStart {
                reason: RefusalReason::UnresolvableGlob { pattern },
            })
        }
        Err(other) => Err(SandboxError::Policy(other)),
    }
}

/// The platform-native backend for the current OS.
#[must_use]
pub fn platform_backend() -> &'static dyn SandboxBackend {
    #[cfg(windows)]
    {
        &crate::backends::windows_sandbox::WINDOWS_SANDBOX
    }
    #[cfg(target_os = "macos")]
    {
        &crate::backends::macos_sandbox::MACOS_SANDBOX
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        &crate::backends::linux_sandbox::LINUX_SANDBOX
    }
}

#[cfg(feature = "container")]
pub mod container;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux_sandbox;
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod linux_seccomp;
pub mod macos_sandbox;
#[cfg(windows)]
pub(crate) mod win_ffi;
#[cfg(windows)]
pub(crate) mod win_spawn;
#[cfg(windows)]
pub(crate) mod win_util;
#[cfg(windows)]
pub mod windows_sandbox;
