//! The typed surface of the host API: log levels, audit events, denial
//! reasons, and the grant a host supplies to one plugin (scopes plus
//! capabilities). Pure types - no engine dependency - so they compile in
//! every build configuration.

use std::path::PathBuf;

use crate::manifest::{Capability, PreopenMode};

/// Severity of a plugin log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Informational.
    Info,
    /// Warning.
    Warn,
    /// Error.
    Error,
}

/// One bounded plugin log line (lossy UTF-8: plugin output is untrusted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Severity the plugin asked for.
    pub level: LogLevel,
    /// The (bounded, lossily decoded) line text.
    pub text: String,
}

/// Why a host file call was denied, typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    /// No scope with that name is granted (or the path has no scope prefix).
    OutsideScopes,
    /// The path resolves outside the scope root (`..`, absolute, symlinks).
    Escape,
    /// The scope is read-only and a write was attempted.
    ReadOnlyScope,
    /// The target does not exist.
    NotFound,
    /// Filesystem error on the host side.
    Io,
    /// The payload exceeds the region/buffer capacity.
    TooLarge,
    /// The guest pointer/length does not address valid memory.
    BadPointer,
}

/// Observable host-side activity for one call (audit surface).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    /// A file inside a scope was read.
    ReadOk {
        /// Virtual path as the plugin spelled it.
        path: String,
        /// Bytes served.
        bytes: usize,
    },
    /// A read was denied.
    ReadDenied {
        /// Virtual path as the plugin spelled it.
        path: String,
        /// Why.
        reason: DenyReason,
    },
    /// A file inside an RW scope was written.
    WriteOk {
        /// Virtual path as the plugin spelled it.
        path: String,
        /// Bytes written.
        bytes: usize,
    },
    /// A write was denied.
    WriteDenied {
        /// Virtual path as the plugin spelled it.
        path: String,
        /// Why.
        reason: DenyReason,
    },
    /// A log line was dropped because the per-call log budget was spent.
    LogDropped {
        /// Why (fixed: budget exhaustion).
        reason: &'static str,
    },
}

/// The ABI-v1 negative return code for a denial reason.
pub(crate) fn deny_code(reason: DenyReason) -> i32 {
    match reason {
        DenyReason::OutsideScopes => -1,
        DenyReason::Escape => -2,
        DenyReason::ReadOnlyScope => -3,
        DenyReason::NotFound | DenyReason::Io => -4,
        DenyReason::TooLarge => -5,
        DenyReason::BadPointer => -6,
    }
}

/// A preopen scope the host actually granted: logical name, host directory,
/// and the maximum mode the host allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedScope {
    /// Logical name the plugin addresses the scope by.
    pub name: String,
    /// The host directory the scope maps to.
    pub host_dir: PathBuf,
    /// Maximum access the host grants.
    pub mode: PreopenMode,
}

/// What the host grants one plugin: capabilities and scopes. Grants start
/// empty; everything a plugin wants must be declared AND granted.
#[derive(Debug, Clone, Default)]
pub struct PluginGrant {
    /// Capabilities the host is willing to honor for this plugin.
    pub capabilities: Vec<Capability>,
    /// Preopen scopes the host supplies, keyed by logical name.
    pub scopes: Vec<ResolvedScope>,
}
