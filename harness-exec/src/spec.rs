//! Typed execution spec and outcome (plan todo 19).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::spill::SpilledOutput;

/// Hard ceiling for any single command's timeout: no unbounded subprocess
/// can exist by construction.
pub const MAX_COMMAND_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

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
    /// Mandatory timeout; clamped by [`MAX_COMMAND_TIMEOUT`].
    pub timeout: Duration,
    /// Terminal geometry.
    pub size: ScreenSize,
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
    /// An I/O failure outside spawn.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
