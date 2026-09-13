//! Typed pre-flight errors and report surface (plan todo 22).
//!
//! The single-slot doctrine (T12): a failed pipeline surfaces ONE
//! composite [`PreFlightFailure`] — primary structured error first, a
//! bounded tail, and a total count. Failed corrective iterations REPLACE
//! the slot; nothing stacks.

use harness_ast::AstError;
use harness_vfs::VfsError;
use std::path::PathBuf;

use super::fixer::FixerError;
use super::structured::StructuredError;

/// The three deterministic gates, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// (a) touched files parse with zero NEW error nodes.
    AstGate,
    /// (b) claiming formatter adapters report the tree formatted.
    Format,
    /// (c) claiming static checkers report zero errors.
    Check,
}

/// Outcome of one stage attempt inside one iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    Passed,
    Failed,
}

/// One reached stage, tagged with its corrective iteration (0 = initial).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageAttempt {
    pub iteration: usize,
    pub stage: Stage,
    pub outcome: AttemptOutcome,
}

/// Why a tool did not run. Skips are notices, never failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// Probe could not spawn the tool (not installed / not on PATH).
    ToolAbsent,
    /// No tree-sitter grammar for the file; the deterministic
    /// balanced-delimiter heuristic ran instead.
    NoGrammarFallback,
    /// Content is not UTF-8 text; no deterministic check applies.
    NotText,
    /// Ecosystem marker file missing (no Cargo.toml / package.json).
    NoMarker,
    /// More than [`super::MAX_FORMAT_FILES`] files; the argument list was
    /// capped and the overflow disclosed.
    FileCap { capped: usize },
}

/// A disclosed skip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipNotice {
    pub stage: Stage,
    pub tool: &'static str,
    pub reason: SkipReason,
}

/// The single volatile error slot's value: one composite, primary first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreFlightFailure {
    pub stage: Stage,
    pub primary: StructuredError,
    /// Bounded tail behind the primary (≤ [`structured::MAX_SURFACED_ERRORS`] − 1).
    pub rest: Vec<StructuredError>,
    /// Total structured errors found (the tail beyond the cap is counted,
    /// never dumped).
    pub total_errors: usize,
    /// The corrective iteration that produced THIS failure.
    pub produced_at_iteration: usize,
}

/// Final verdict of a pipeline run.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// All gates passed; the shadow is commit-ready.
    Passed,
    /// The slot's composite failure after the corrective cycle.
    Failed(PreFlightFailure),
}

/// The full deterministic record of a pipeline run.
#[derive(Debug, Clone, PartialEq)]
pub struct PreFlightReport {
    pub outcome: Outcome,
    /// Fixer invocations consumed (≤ [`super::MAX_FIX_ITERATIONS`]).
    pub fix_iterations: usize,
    /// Every reached stage attempt, in order (the misleading-success
    /// guard: assertions pin this sequence).
    pub trace: Vec<StageAttempt>,
    /// Every disclosed skip.
    pub skips: Vec<SkipNotice>,
}

/// Infrastructural failures (gate outcomes are NOT errors — they are the
/// `Failed` report path).
#[derive(Debug, thiserror::Error)]
pub enum PreFlightError {
    /// Cancellation observed between stages or inside a runner; the run
    /// aborted cleanly, the shadow is intact and retryable.
    #[error("pre-flight cancelled")]
    Cancelled,
    /// A touched file could not be read from the shadow/user tree.
    #[error("reading touched file {path} failed: {source}")]
    ReadTouched {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The analyzer rejected a file (size cap, language setup).
    #[error("analyzing {path} failed: {source}")]
    Analyze { path: PathBuf, source: AstError },
    /// The injected fixer policy failed.
    #[error("fixer policy failed: {source}")]
    Fixer { source: FixerError },
    /// Corrective edits could not be revised into the shadow.
    #[error("revising corrective edits failed: {source}")]
    Revise { source: VfsError },
}
