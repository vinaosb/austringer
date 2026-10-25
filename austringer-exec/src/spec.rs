//! Typed execution spec and outcome (plan todo 19).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::spill::SpilledOutput;
use crate::wsl::WslStatus;

/// Hard ceiling for any single command's timeout: no unbounded subprocess
/// can exist by construction.
pub const MAX_COMMAND_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// Where a command executes (todo 20 WSL2 bridge).
///
/// `Native` (the default) spawns the program directly and NEVER touches
/// WSL — a WSL-free machine is unaffected. `Wsl` routes through
/// `wsl.exe --exec` ([`crate::wsl`]); when WSL is unavailable that is a
/// typed [`ExecError::WslUnavailable`], not an error on default
/// operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunIn {
    /// Direct spawn on the host.
    #[default]
    Native,
    /// `wsl.exe --exec <program> <args…>` with drive paths mapped to
    /// their `/mnt` equivalents.
    Wsl,
}

/// Terminal geometry; the plan default is 120 columns by 40 rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    /// Grid columns.
    pub cols: u16,
    /// Grid rows.
    pub rows: u16,
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self {
            cols: 120,
            rows: 40,
        }
    }
}

/// One fully-specified command execution through the PTY pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecSpec {
    /// Program to run.
    pub program: String,
    /// Arguments (no implicit shell).
    pub args: Vec<String>,
    /// Working directory (inherited when `None`).
    pub cwd: Option<PathBuf>,
    /// Environment additions layered over the inherited environment.
    pub env: BTreeMap<String, String>,
    /// When set, variables the scrub drops (KEY/SECRET/TOKEN minus its
    /// exceptions, CORE_VARS always kept) are removed from the child's
    /// INHERITED environment and the additions — the safety policy's
    /// env scrub applied at this spawn seam (todo 4).
    pub env_scrub: Option<austringer_safety::scrub::EnvScrub>,
    /// Mandatory timeout; clamped by [`MAX_COMMAND_TIMEOUT`].
    pub timeout: Duration,
    /// Terminal geometry.
    pub size: ScreenSize,
    /// Execution side: host or WSL2 bridge (todo 20).
    pub run_in: RunIn,
}

/// How a command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecStatus {
    /// The child exited on its own.
    Completed {
        /// Its exit code.
        exit_code: u32,
    },
    /// The timeout ceiling fired and the tree was killed.
    TimedOut,
    /// Cancellation was requested and the tree was killed.
    Cancelled,
}

/// The model-facing result of one command. Structurally contains no raw
/// byte stream: only bounded windowed text plus an optional spill pointer.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecOutcome {
    /// How the command ended.
    pub status: ExecStatus,
    /// Bounded, windowed, ANSI-free rendering (the ONLY text the model
    /// ever sees).
    pub model_text: String,
    /// Set when the raw stream spilled to disk.
    pub spill: Option<SpilledOutput>,
    /// Total logical transcript lines before windowing.
    pub total_lines: u64,
    /// Wall-clock duration of the run.
    pub elapsed: Duration,
    /// The PTY reader thread finished (EOF observed) before teardown
    /// completed. `false` means a still-blocked reader was detached as a
    /// documented last resort — it leaks one thread until conhost exits,
    /// which the todo-20 teardown makes unreachable in practice.
    pub reader_joined: bool,
}

/// Typed execution failures.
#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    /// The spec's timeout is zero or beyond the ceiling.
    #[error("invalid timeout {requested:?}: must be > 0 and <= {MAX_COMMAND_TIMEOUT:?}")]
    InvalidTimeout {
        /// The offending requested timeout.
        requested: Duration,
    },
    /// The program could not be spawned.
    #[error("spawn `{program}` failed: {source}")]
    Spawn {
        /// The program that failed.
        program: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// WSL routing was requested on a machine that cannot provide it
    /// (typed: default operation never triggers this).
    #[error("WSL routing unavailable: {status}")]
    WslUnavailable {
        /// Why WSL is not usable here.
        status: WslStatus,
    },
    /// An I/O failure outside spawn.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<WslStatus> for ExecError {
    fn from(status: WslStatus) -> Self {
        Self::WslUnavailable { status }
    }
}
