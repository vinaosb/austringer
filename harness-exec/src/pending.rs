//! Deferred batch execution (plan todo 19, Plandex `_apply.sh` pattern).
//!
//! Commands can accumulate in a pending change-set alongside file edits
//! and apply ATOMICALLY on approval: [`PendingCommandSet::apply`] runs
//! them sequentially through any [`CommandRunner`], stops at the first
//! command that does not complete, and leaves the remainder queued for a
//! retry or [`PendingCommandSet::abort`]. This is an ADDITIVE mode —
//! live execution via [`crate::runner::PtyRunner`] remains the primary
//! path and both share the same runner seam.

use crate::runner::CancelToken;
use crate::spec::{ExecError, ExecOutcome, ExecSpec, ExecStatus};

/// What `apply` ran before stopping. `remaining` counts commands that
/// never executed (the failed command itself counts as executed).
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyReport {
    /// Index + status of every command that ran.
    pub executed: Vec<(usize, ExecStatus)>,
    /// Index of the first command that did not complete successfully
    /// (spawn error, timeout, cancellation, or nonzero exit).
    pub failed_at: Option<usize>,
    /// Commands left unexecuted; they stay queued on the set.
    pub remaining: usize,
}

/// Live-execution seam so batch semantics stay testable without a PTY.
pub trait CommandRunner {
    /// Runs one spec; cancellation flows through.
    fn run(&self, spec: &ExecSpec, cancel: &CancelToken) -> Result<ExecOutcome, ExecError>;
}

impl CommandRunner for crate::runner::PtyRunner {
    fn run(&self, spec: &ExecSpec, cancel: &CancelToken) -> Result<ExecOutcome, ExecError> {
        self.run_with_cancel(spec, cancel)
    }
}

/// Accumulates commands for review, then applies them atomically on
/// approval.
#[derive(Debug, Default)]
pub struct PendingCommandSet {
    commands: Vec<ExecSpec>,
}

impl PendingCommandSet {
    /// An empty change-set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues one command.
    pub fn push(&mut self, spec: ExecSpec) -> &mut Self {
        self.commands.push(spec);
        self
    }

    /// Queued commands, in order (for review/inspection).
    #[must_use]
    pub fn commands(&self) -> &[ExecSpec] {
        &self.commands
    }

    /// Applies every queued command through `runner`, stopping at the
    /// first non-successful result. On failure or cancellation the
    /// remaining commands stay queued. A pre-cancelled token runs
    /// nothing.
    pub fn apply(&mut self, runner: &dyn CommandRunner, cancel: &CancelToken) -> ApplyReport {
        if cancel.is_cancelled() {
            return ApplyReport {
                executed: Vec::new(),
                failed_at: None,
                remaining: self.commands.len(),
            };
        }
        let mut executed = Vec::new();
        let mut failed_at = None;
        for (index, spec) in self.commands.iter().enumerate() {
            let status = match runner.run(spec, cancel) {
                Ok(outcome) => outcome.status,
                Err(_) => {
                    failed_at = Some(index);
                    break;
                }
            };
            let completed = status == ExecStatus::Completed { exit_code: 0 };
            executed.push((index, status));
            if !completed {
                failed_at = Some(index);
                break;
            }
        }
        ApplyReport {
            remaining: self.commands.len() - executed.len(),
            executed,
            failed_at,
        }
    }

    /// Discards the queue, returning the aborted commands.
    pub fn abort(&mut self) -> Vec<ExecSpec> {
        std::mem::take(&mut self.commands)
    }
}
