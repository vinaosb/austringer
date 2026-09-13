//! The deterministic pre-flight pipeline (plan todo 22): AST gate →
//! formatter adapters → static-check adapters, all running INSIDE a T21
//! shadow, with a closed corrective sub-cycle (≤ [`MAX_FIX_ITERATIONS`])
//! through an injected [`FixerSeam`] before surfacing.
//!
//! # Determinism
//!
//! No LLM anywhere: every gate is a parse, a probe, or a bounded command
//! through the T19 [`CommandRunner`] seam (production: `PtyRunner` with
//! timeout ceilings and kill trees; tests: scripted runners). The fixer
//! seam is injected policy; the shipped default [`SurfaceAfterTwo`]
//! never fixes, so the default pipeline surfaces after the initial run —
//! the ≤2 cap holds trivially because an empty fix set cannot change a
//! provably identical rerun.
//!
//! # Single volatile error slot (T12 doctrine)
//!
//! A failed iteration produces ONE composite
//! [`PreFlightFailure`][error::PreFlightFailure] — primary structured
//! error first, bounded tail, total count. The next failed iteration
//! REPLACES it; errors never stack. The surfaced rejection is compact by
//! construction: file:line + primary message only.
//!
//! # Stage order
//!
//! `(a)` AST gate over every touched file (zero NEW error nodes, T14
//! diagnostics; no-grammar files degrade to the documented
//! balanced-delimiter heuristic) — a syntax error stops the pipeline
//! before ANY formatter or compiler runs. `(b)` formatter adapters
//! (rustfmt/prettier/gofmt/black) probe for availability; absent tools
//! are typed skips, never failures. `(c)` static checkers (cargo check
//! --message-format=json, tsc --noEmit) with structured extraction.
//!
//! The pipeline never rewrites the shadow: corrections flow exclusively
//! through the fixer seam's `ShadowHandle::revise`, and the caller
//! commits only after `Outcome::Passed`.

pub mod setup;

pub(crate) mod astgate;
pub(crate) mod check;
pub(crate) mod error;
pub(crate) mod fixer;
pub(crate) mod format;
pub(crate) mod jsonscan;
pub(crate) mod setup_job;
pub(crate) mod setup_parse;
pub(crate) mod stages;
pub(crate) mod structured;

use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_ast::Analyzer;

use crate::cancel::CancelToken;
use crate::pending::CommandRunner;
use crate::spec::ScreenSize;

pub use error::{
    AttemptOutcome, Outcome, PreFlightError, PreFlightFailure, PreFlightReport, SkipNotice,
    SkipReason, Stage, StageAttempt,
};
pub use fixer::{FixerError, FixerSeam, SurfaceAfterTwo};
pub use harness_vfs::ShadowEdit;
pub use structured::StructuredError;

pub use check::{CargoCheck, StaticChecker, TscCheck};
pub use format::{Black, FormatterAdapter, Gofmt, Prettier, Rustfmt};

/// Maximum corrective fixer invocations before surfacing.
pub const MAX_FIX_ITERATIONS: usize = 2;

/// Maximum files passed to one formatter invocation (bounded command
/// line); the overflow is disclosed as a [`SkipReason::FileCap`] notice.
pub const MAX_FORMAT_FILES: usize = 64;

/// Inputs for one tool invocation (probe- and check-spec construction).
pub struct ToolContext<'a> {
    pub root: &'a Path,
    pub files: &'a [PathBuf],
    pub timeout: Duration,
    pub size: ScreenSize,
}

/// Per-run knobs. Timeouts are honored by the T19 runner machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreFlightConfig {
    pub format_timeout: Duration,
    pub check_timeout: Duration,
}

impl Default for PreFlightConfig {
    fn default() -> Self {
        Self {
            format_timeout: Duration::from_secs(60),
            check_timeout: Duration::from_secs(300),
        }
    }
}

/// Everything the deterministic pipeline needs, injected.
pub struct PreFlightDeps<'a> {
    pub runner: &'a dyn CommandRunner,
    pub analyzer: &'a Analyzer,
    pub cancel: &'a CancelToken,
    pub formatters: Vec<Box<dyn FormatterAdapter>>,
    pub checkers: Vec<Box<dyn StaticChecker>>,
    pub config: PreFlightConfig,
}

/// The shipped formatter registry (rustfmt, prettier, gofmt, black).
#[must_use]
pub fn default_formatters() -> Vec<Box<dyn FormatterAdapter>> {
    format::default_formatters()
}

/// The shipped checker registry (cargo check, tsc).
#[must_use]
pub fn default_checkers() -> Vec<Box<dyn StaticChecker>> {
    check::default_checkers()
}

/// One pre-flight run over a shadow.
pub struct PreFlight<'a> {
    deps: &'a PreFlightDeps<'a>,
    fixer: &'a mut dyn FixerSeam,
}

impl<'a> PreFlight<'a> {
    #[must_use]
    pub fn new(deps: &'a PreFlightDeps<'a>, fixer: &'a mut dyn FixerSeam) -> Self {
        Self { deps, fixer }
    }

    /// Run the gates over `touched` (shadow-relative). On pass, the
    /// shadow is commit-ready (the CALLER commits; this never writes the
    /// user tree).
    pub fn run(
        &mut self,
        shadow: &mut harness_vfs::ShadowHandle,
        touched: &[PathBuf],
    ) -> Result<PreFlightReport, PreFlightError> {
        let mut trace = Vec::new();
        let mut skips = Vec::new();
        let mut fix_iterations = 0;
        let mut iteration = 0;
        let outcome = loop {
            self.check_cancel()?;
            match self.stages(shadow, touched, iteration, &mut trace, &mut skips)? {
                Some(failure) => {
                    // Single volatile slot: the LATEST failure replaces
                    // any earlier one; nothing accumulates.
                    if fix_iterations >= MAX_FIX_ITERATIONS {
                        break Outcome::Failed(failure);
                    }
                    let edits = self
                        .fixer
                        .fix(&failure, shadow)
                        .map_err(|source| PreFlightError::Fixer { source })?;
                    fix_iterations += 1;
                    if edits.is_empty() {
                        break Outcome::Failed(failure);
                    }
                    shadow
                        .revise(&edits)
                        .map_err(|source| PreFlightError::Revise { source })?;
                    iteration += 1;
                }
                None => break Outcome::Passed,
            }
        };
        Ok(PreFlightReport {
            outcome,
            fix_iterations,
            trace,
            skips,
        })
    }

    fn check_cancel(&self) -> Result<(), PreFlightError> {
        if self.deps.cancel.is_cancelled() {
            return Err(PreFlightError::Cancelled);
        }
        Ok(())
    }

    /// One full pass of the three stages; `Some(failure)` = the slot
    /// value for this iteration.
    fn stages(
        &mut self,
        shadow: &mut harness_vfs::ShadowHandle,
        touched: &[PathBuf],
        iteration: usize,
        trace: &mut Vec<StageAttempt>,
        skips: &mut Vec<SkipNotice>,
    ) -> Result<Option<PreFlightFailure>, PreFlightError> {
        let root = shadow.shadow_path().to_path_buf();
        let target = shadow.target().to_path_buf();

        let (errors, notices) = astgate::run(&root, &target, touched, self.deps.analyzer)?;
        skips.extend(notices);
        if !errors.is_empty() {
            return Ok(Some(self.close_stage(
                Stage::AstGate,
                errors,
                iteration,
                trace,
            )));
        }
        trace.push(stages::passed(Stage::AstGate, iteration));
        self.check_cancel()?;

        let mut run = stages::StageRun {
            root: &root,
            touched,
            iteration,
            trace,
            skips,
        };
        if let Some(failure) = self.format_stage(&mut run)? {
            return Ok(Some(failure));
        }
        self.check_cancel()?;
        self.check_stage(&mut run)
    }

    pub(super) fn close_stage(
        &self,
        stage: Stage,
        errors: Vec<StructuredError>,
        iteration: usize,
        trace: &mut Vec<StageAttempt>,
    ) -> PreFlightFailure {
        trace.push(StageAttempt {
            iteration,
            stage,
            outcome: AttemptOutcome::Failed,
        });
        structured::build_failure(stage, errors, iteration)
    }
}
