//! Executor-lane type vocabulary (plan M15): availability, the typed
//! failure surface (richer than [`MediaError`] - the audit-carrying
//! SandboxDenied lives here), and the completed-run record with its
//! producer disclosure.

use std::path::PathBuf;

use austringer_safety::policy::decl::ViolationEvent;

use crate::error::{InstallFailure, MediaError};
use crate::executor::{AdapterTier, ProducerInfo};

/// Why a tool is (not) runnable right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolAvailability {
    /// Enabled and the program resolves on this machine.
    Available {
        /// Resolved program path.
        program: PathBuf,
        /// The manifest version pin (producer disclosure).
        version_pin: String,
    },
    /// Not enabled or not installed; `install_hint` is actionable.
    Unavailable {
        /// How to make it available.
        install_hint: String,
    },
}

/// Typed subprocess failures - the executor's richer surface over
/// [`MediaError`] (the audit-carrying SandboxDenied lives here).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    /// Not enabled / not installed / no manifest.
    #[error("external tool {tool} is unavailable: {install_hint}")]
    ToolUnavailable {
        /// Tool name.
        tool: String,
        /// Actionable next step.
        install_hint: String,
    },
    /// The install gate refused (sha mismatch, malformed pin, fetch).
    #[error("pinned asset for {tool} failed the install gate: {reason:?}")]
    InstallFailed {
        /// Tool name.
        tool: String,
        /// Which discipline failed.
        reason: InstallFailure,
    },
    /// The sandbox policy gate denied the run; `violation` is the audit.
    #[error("sandbox denied {tool}: {}", violation.detail)]
    SandboxDenied {
        /// Tool name.
        tool: String,
        /// The austringer-safety audit record.
        violation: ViolationEvent,
    },
    /// The invocation exceeded its deadline and the tree was killed.
    #[error("external tool {tool} exceeded its timeout; the process tree was killed")]
    Timeout {
        /// Tool name.
        tool: String,
    },
    /// Non-zero exit with the bounded stderr head.
    #[error("external tool {tool} exited {code}: {stderr_head}")]
    NonZeroExit {
        /// Tool name.
        tool: String,
        /// Exit code.
        code: i32,
        /// Bounded stderr head.
        stderr_head: String,
    },
    /// The OS refused the spawn.
    #[error("external tool {tool} failed to spawn: {detail}")]
    Spawn {
        /// Tool name.
        tool: String,
        /// What the OS said.
        detail: String,
    },
}

impl From<ToolError> for MediaError {
    fn from(error: ToolError) -> Self {
        match error {
            ToolError::ToolUnavailable { tool, install_hint } => {
                MediaError::ToolUnavailable { tool, install_hint }
            }
            ToolError::InstallFailed { reason, .. } => MediaError::InstallFailed { reason },
            ToolError::SandboxDenied { .. } => MediaError::SandboxDenied,
            ToolError::Timeout { .. } => MediaError::Timeout,
            ToolError::NonZeroExit {
                code, stderr_head, ..
            } => MediaError::NonZeroExit { code, stderr_head },
            ToolError::Spawn { tool, detail } => {
                let install_hint = format!("{tool} failed to spawn: {detail}");
                MediaError::ToolUnavailable { tool, install_hint }
            }
        }
    }
}

/// A completed tool run: bounded outputs plus the disclosure fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRun {
    /// Tool name.
    pub tool: String,
    /// Manifest version pin.
    pub version_pin: String,
    /// Exit code (0 here by construction).
    pub exit_code: i32,
    /// Bounded stdout.
    pub stdout: Vec<u8>,
    /// Bounded stderr head.
    pub stderr_head: String,
    /// The executed command line.
    pub command: String,
}

impl ToolRun {
    /// The standing-constraint-4 disclosure for this artifact.
    #[must_use]
    pub fn producer(&self) -> ProducerInfo {
        ProducerInfo {
            tier: AdapterTier::Subprocess,
            engine: self.tool.clone(),
            version: self.version_pin.clone(),
            command: Some(self.command.clone()),
        }
    }
}
