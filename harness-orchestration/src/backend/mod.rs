//! Harness-as-subagent backends (dsh precedent, research SS6): delegating
//! tasks to real Claude Code / Codex child processes over their official
//! stdio protocols, as additional launcher backends.
//!
//! SUPPLY-CHAIN CARVE-OUT: platform binaries are opt-in and DISABLED BY
//! DEFAULT — no external binary is ever spawned, downloaded, or trusted
//! unless the operator explicitly enables a backend.

pub mod install;
pub mod plugins;
pub mod process;
pub mod protocol;

use crate::authorize::CommandExecutionPolicy;

/// Which external harness a backend drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    /// Claude Code (`claude app-server --stdio`).
    ClaudeCode,
    /// Codex (`codex app-server`).
    Codex,
}

impl BackendKind {
    /// The vendor's documented protocol command.
    pub fn protocol_command(self) -> (&'static str, Vec<&'static str>) {
        match self {
            Self::ClaudeCode => ("claude", vec!["app-server", "--stdio"]),
            Self::Codex => ("codex", vec!["app-server"]),
        }
    }
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        })
    }
}

/// Backend configuration. Default = disabled (the carve-out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendConfig {
    /// Whether the backend may spawn its child process at all.
    pub enabled: bool,
    /// Program to run (the vendor binary or an override).
    pub program: String,
    /// Program arguments.
    pub args: Vec<String>,
    /// Command-execution policy applied inside the child.
    pub policy: CommandExecutionPolicy,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            program: String::new(),
            args: Vec::new(),
            policy: CommandExecutionPolicy::Auto,
        }
    }
}

impl BackendConfig {
    /// The explicit opt-in: names the program and enables the backend.
    pub fn opt_in(program: String) -> Self {
        Self {
            enabled: true,
            program,
            args: Vec::new(),
            policy: CommandExecutionPolicy::Auto,
        }
    }

    /// Applies the vendor's documented protocol arguments for a kind.
    pub fn with_protocol(mut self, kind: BackendKind) -> Self {
        let (program, args) = kind.protocol_command();
        if self.program.is_empty() {
            self.program = program.to_string();
        }
        self.args = args.into_iter().map(String::from).collect();
        self
    }
}
