//! The tool stages (b) and (c) of the pre-flight pipeline (plan todo
//! 22): formatter-adapter and static-checker orchestration over the
//! injected [`CommandRunner`], probe-based availability detection, and
//! the bounded outcome→structured-error mapping.

use std::path::PathBuf;
use std::time::Duration;

use super::error::{
    AttemptOutcome, PreFlightError, PreFlightFailure, SkipNotice, SkipReason, Stage, StageAttempt,
};
use super::structured::StructuredError;
use super::{MAX_FORMAT_FILES, PreFlight, ToolContext};
use crate::spec::{ExecOutcome, ExecSpec, ExecStatus, ScreenSize};

/// Mutable bookkeeping one stage pass shares (trace + skips + loop
/// constants), grouped to keep the stage functions two-parameter.
pub(super) struct StageRun<'a> {
    pub(super) root: &'a std::path::Path,
    pub(super) touched: &'a [PathBuf],
    pub(super) iteration: usize,
    pub(super) trace: &'a mut Vec<StageAttempt>,
    pub(super) skips: &'a mut Vec<SkipNotice>,
}

impl PreFlight<'_> {
    pub(super) fn format_stage(
        &mut self,
        run: &mut StageRun<'_>,
    ) -> Result<Option<PreFlightFailure>, PreFlightError> {
        let mut errors = Vec::new();
        for adapter in &self.deps.formatters {
            self.check_cancel()?;
            let claimed: Vec<PathBuf> = run
                .touched
                .iter()
                .filter(|rel| adapter.claims(rel))
                .cloned()
                .collect();
            if claimed.is_empty() {
                continue;
            }
            let probe = adapter.probe_spec(run.root);
            if !self.probe_present(Stage::Format, adapter.tool(), probe, run.skips)? {
                continue;
            }
            if claimed.len() > MAX_FORMAT_FILES {
                run.skips.push(SkipNotice {
                    stage: Stage::Format,
                    tool: adapter.tool(),
                    reason: super::format::file_cap_notice(claimed.len() - MAX_FORMAT_FILES),
                });
            }
            let ctx = ToolContext {
                root: run.root,
                files: &claimed,
                timeout: self.deps.config.format_timeout,
                size: machine_size(),
            };
            let spec = adapter.check_spec(&ctx);
            let Some(outcome) = self.run_spec(spec) else {
                run.skips.push(SkipNotice {
                    stage: Stage::Format,
                    tool: adapter.tool(),
                    reason: SkipReason::ToolAbsent,
                });
                continue;
            };
            let extracted = adapter.extract(&outcome.model_text);
            errors.extend(self.map_outcome(
                Stage::Format,
                adapter.tool(),
                outcome,
                self.deps.config.format_timeout,
                extracted,
            )?);
        }
        if errors.is_empty() {
            run.trace.push(passed(Stage::Format, run.iteration));
            return Ok(None);
        }
        Ok(Some(self.close_stage(
            Stage::Format,
            errors,
            run.iteration,
            run.trace,
        )))
    }

    pub(super) fn check_stage(
        &mut self,
        run: &mut StageRun<'_>,
    ) -> Result<Option<PreFlightFailure>, PreFlightError> {
        let mut errors = Vec::new();
        for adapter in &self.deps.checkers {
            self.check_cancel()?;
            if !run.touched.iter().any(|rel| adapter.claims(rel)) {
                continue;
            }
            if !adapter.marker(run.root) {
                run.skips.push(SkipNotice {
                    stage: Stage::Check,
                    tool: adapter.tool(),
                    reason: super::check::no_marker_notice(),
                });
                continue;
            }
            let probe = adapter.probe_spec(run.root);
            if !self.probe_present(Stage::Check, adapter.tool(), probe, run.skips)? {
                continue;
            }
            let ctx = ToolContext {
                root: run.root,
                files: run.touched,
                timeout: self.deps.config.check_timeout,
                size: machine_size(),
            };
            let spec = adapter.check_spec(&ctx);
            let Some(outcome) = self.run_spec(spec) else {
                run.skips.push(SkipNotice {
                    stage: Stage::Check,
                    tool: adapter.tool(),
                    reason: SkipReason::ToolAbsent,
                });
                continue;
            };
            let extracted = adapter.extract(&outcome.model_text);
            errors.extend(self.map_outcome(
                Stage::Check,
                adapter.tool(),
                outcome,
                self.deps.config.check_timeout,
                extracted,
            )?);
        }
        if errors.is_empty() {
            run.trace.push(passed(Stage::Check, run.iteration));
            return Ok(None);
        }
        Ok(Some(self.close_stage(
            Stage::Check,
            errors,
            run.iteration,
            run.trace,
        )))
    }

    /// Probe: spawn success (any status) = present; spawn failure =
    /// typed skip. A cancelled probe aborts the run.
    pub(super) fn probe_present(
        &self,
        stage: Stage,
        tool: &'static str,
        probe: ExecSpec,
        skips: &mut Vec<SkipNotice>,
    ) -> Result<bool, PreFlightError> {
        match self.deps.runner.run(&probe, self.deps.cancel) {
            Ok(outcome) if outcome.status == ExecStatus::Cancelled => {
                Err(PreFlightError::Cancelled)
            }
            Ok(_) => Ok(true),
            Err(_) => {
                skips.push(SkipNotice {
                    stage,
                    tool,
                    reason: SkipReason::ToolAbsent,
                });
                Ok(false)
            }
        }
    }

    pub(super) fn run_spec(&self, spec: ExecSpec) -> Option<ExecOutcome> {
        // A spawn/io failure after a successful probe means the tool
        // became unavailable mid-run; disclosed as a skip, never a
        // failure.
        self.deps.runner.run(&spec, self.deps.cancel).ok()
    }

    pub(super) fn map_outcome(
        &self,
        stage: Stage,
        tool: &'static str,
        outcome: ExecOutcome,
        timeout: Duration,
        mut errors: Vec<StructuredError>,
    ) -> Result<Vec<StructuredError>, PreFlightError> {
        let generic = |message: String| {
            vec![StructuredError {
                stage,
                tool,
                path: None,
                line: None,
                column: None,
                message,
            }]
        };
        match outcome.status {
            ExecStatus::Completed { exit_code: 0 } => Ok(Vec::new()),
            ExecStatus::Completed { exit_code } => {
                if errors.is_empty() {
                    errors = generic(format!(
                        "exited with code {exit_code} without structured output"
                    ));
                }
                Ok(errors)
            }
            ExecStatus::TimedOut => {
                Ok(generic(format!("{tool} check timed out after {timeout:?}")))
            }
            ExecStatus::Cancelled => Err(PreFlightError::Cancelled),
        }
    }
}

pub(super) fn passed(stage: Stage, iteration: usize) -> StageAttempt {
    StageAttempt {
        iteration,
        stage,
        outcome: AttemptOutcome::Passed,
    }
}

fn machine_size() -> ScreenSize {
    ScreenSize {
        cols: 1000,
        rows: 60,
    }
}
