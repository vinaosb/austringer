//! The agent loop: configuration, typed run outcomes, and the pipeline
//! driver (iteration loop, cancel marks, high-water records).

use std::collections::VecDeque;
use std::io;
use std::path::Path;

use super::cancel::{CancelHandle, CancelRecord, CancelToken};
use super::config::{FailureReason, LoopConfig, RunOutcome};
use super::dispatch::OpAction;
use super::focus::{CompletionBlock, FocusChain, PlanGate};
use super::inference::{Inference, InferenceError, PendingCall};
use super::operation::Operation;
use super::stages::StageSet;
use super::status::SessionStatus;
use crate::event::{Event, RecordedEvent, Sequence};
use crate::log::{EventLogWriter, FsyncPolicy, replay};

/// The agent loop: a named-operation pipeline (see [`Operation::ALL`]) over
/// the append-only event log — never an unstructured while-turn.
///
/// Cancellation lands between/at operations via cancel-marks and high-water
/// sequences: after a cancel the session resumes from a consistent
/// boundary with no half-written operations beyond the mark. Opening an
/// existing session directory IS resume (torn tails are handled by the
/// writer; in-flight tool calls are rebuilt from unanswered trailing
/// tool-call events).
pub struct AgentLoop {
    pub(super) log: EventLogWriter,
    pub(super) events: Vec<RecordedEvent>,
    pub(super) status: SessionStatus,
    pub(super) config: LoopConfig,
    pub(super) stages: StageSet,
    pub(super) inference: Box<dyn Inference>,
    pub(super) token: CancelToken,
    pub(super) focus: FocusChain,
    pub(super) plan_gate: PlanGate,
    pub(super) steer_queue: VecDeque<String>,
    pub(super) pending_calls: Vec<PendingCall>,
    pub(super) force_inference: bool,
    pub(super) steered: bool,
    pub(super) turn_ended: bool,
    pub(super) turn: u32,
    pub(super) stop_blocks: u32,
    pub(super) failures: u32,
    pub(super) last_error: Option<InferenceError>,
    pub(super) last_cancel: Option<CancelRecord>,
    pub(super) last_completion_block: Option<CompletionBlock>,
}

impl AgentLoop {
    /// Opens (or resumes) a session directory. Rebuilding: events replay
    /// from disk, and trailing tool calls without results become the
    /// in-flight set again.
    pub fn open(
        session_dir: &Path,
        config: LoopConfig,
        stages: StageSet,
        inference: Box<dyn Inference>,
    ) -> io::Result<Self> {
        let log = EventLogWriter::open(session_dir, FsyncPolicy::Always)?;
        let report = replay(session_dir)?;
        let events = report.events;
        let pending_calls = super::tools::rebuild_pending(&events);
        Ok(Self {
            log,
            pending_calls,
            events,
            status: SessionStatus::InProgress,
            config,
            stages,
            inference,
            token: CancelToken::new(),
            focus: FocusChain::new(),
            plan_gate: PlanGate::new(),
            steer_queue: VecDeque::new(),
            force_inference: false,
            steered: false,
            turn_ended: false,
            turn: 0,
            stop_blocks: 0,
            failures: 0,
            last_error: None,
            last_cancel: None,
            last_completion_block: None,
        })
    }

    /// Queues user input; the Steer operation absorbs it into the
    /// conversation on the next iteration.
    pub fn submit(&mut self, text: impl Into<String>) {
        self.steer_queue.push_back(text.into());
    }

    /// An external cancel handle for a running loop.
    #[must_use]
    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle {
            token: self.token.clone(),
        }
    }

    /// The current session status.
    #[must_use]
    pub fn status(&self) -> SessionStatus {
        self.status
    }

    /// Mutable access to the focus chain.
    pub fn focus_mut(&mut self) -> &mut FocusChain {
        &mut self.focus
    }

    /// The plan.md hard-block gate (the daemon wires it; the loop enforces
    /// it on task-level completion).
    #[must_use]
    pub fn plan_gate(&self) -> &PlanGate {
        &self.plan_gate
    }

    /// The in-memory event mirror (log is the durable truth).
    #[must_use]
    pub fn events(&self) -> &[RecordedEvent] {
        &self.events
    }

    /// The last cancel record, when this loop instance cancelled.
    #[must_use]
    pub fn last_cancel(&self) -> Option<&CancelRecord> {
        self.last_cancel.as_ref()
    }

    /// The last typed completion block (why task completion was deferred).
    #[must_use]
    pub fn last_completion_block(&self) -> Option<&CompletionBlock> {
        self.last_completion_block.as_ref()
    }

    /// Swaps the inference seam (tests script follow-up turns; the daemon
    /// swaps provider adapters).
    pub fn set_inference(&mut self, inference: Box<dyn Inference>) {
        self.inference = inference;
    }

    /// Runs the pipeline until the turn ends, the task completes, a cancel
    /// lands, or the session fails.
    pub async fn run(&mut self) -> RunOutcome {
        match self.status {
            SessionStatus::Success | SessionStatus::Failed => {
                return RunOutcome::Failed(FailureReason::Terminal {
                    status: self.status,
                });
            }
            SessionStatus::Cancelled | SessionStatus::Blocked => {
                // Sanctioned resume/re-prompt escapes.
                if let Ok(next) = self.status.transition(SessionStatus::InProgress) {
                    self.status = next;
                }
            }
            SessionStatus::InProgress | SessionStatus::Cancelling => {}
        }
        self.stop_blocks = 0;
        self.failures = 0;
        loop {
            if self.token.is_cancelling() {
                return self.cancel_run(Operation::EntryHook, false);
            }
            let has_work = self.force_inference
                || !self.pending_calls.is_empty()
                || !self.steer_queue.is_empty()
                || self.last_error.is_some()
                || self.turn_ended;
            if !has_work {
                return RunOutcome::TurnComplete;
            }
            self.force_inference = false;
            self.steered = false;
            for operation in Operation::ALL {
                match self.run_operation(operation).await {
                    OpAction::Proceed => {}
                    OpAction::SkipRest => break,
                    OpAction::Fail(reason) => return self.fail_typed(reason),
                    OpAction::Complete => return self.complete_run(),
                    OpAction::TurnComplete => return RunOutcome::TurnComplete,
                    OpAction::Stopped(outcome) => return outcome,
                }
            }
        }
    }

    /// Applies task-level completion after the cancel check cleared it.
    fn complete_run(&mut self) -> RunOutcome {
        if let Ok(success) = self.status.transition(SessionStatus::Success) {
            self.status = success;
        }
        RunOutcome::TaskCompleted
    }

    /// Runs one operation under the cancel-grace race.
    async fn run_operation(&mut self, operation: Operation) -> OpAction {
        if self.token.is_cancelling() {
            return OpAction::Stopped(self.cancel_run(operation, false));
        }
        let grace = self.config.cancel_grace;
        let token = self.token.clone();
        let mut stage = Box::pin(super::dispatch::dispatch(self, operation));
        let completed = tokio::select! {
            biased;
            action = &mut stage => Some(action),
            () = token.cancelled_then(grace) => None,
        };
        drop(stage);
        match completed {
            Some(action) => {
                // A cancel observed while the stage ran wins over whatever
                // the stage concluded; its outcome is discarded.
                if self.token.is_cancelling() {
                    OpAction::Stopped(self.cancel_run(operation, false))
                } else {
                    action
                }
            }
            // Grace expired: the in-flight operation is dropped mid-flight
            // (torn tool executions abort cleanly); nothing further is
            // appended for it.
            None => OpAction::Stopped(self.cancel_run(operation, true)),
        }
    }

    /// Lands a cancel: records the high-water mark event and moves the
    /// status machine through Cancelling to terminal Cancelled.
    fn cancel_run(&mut self, operation: Operation, forced: bool) -> RunOutcome {
        let high_water = self.log.last_sequence();
        if let Ok(cancelling) = self.status.transition(SessionStatus::Cancelling) {
            self.status = cancelling;
        }
        let kind = if forced { "forced" } else { "clean" };
        let mark = Event::System {
            text: format!(
                "SESSION CANCELLED after sequence {} during {} ({kind})",
                high_water.0,
                operation.name()
            ),
        };
        if let Err(error) = self.record(mark) {
            // The log is degraded; from Cancelling the Failed transition
            // is illegal, so the status stays Cancelling while the failure
            // surfaces loudly.
            return self.fail_typed(FailureReason::Stage {
                operation,
                cause: error.to_string(),
            });
        }
        if let Ok(cancelled) = self.status.transition(SessionStatus::Cancelled) {
            self.status = cancelled;
        }
        let record = CancelRecord {
            operation,
            high_water,
            forced,
        };
        self.last_cancel = Some(record);
        RunOutcome::Cancelled(record)
    }

    /// Appends an event to log and mirror, assigning its sequence.
    pub(crate) fn record(&mut self, event: Event) -> Result<Sequence, crate::log::AppendError> {
        let seq = self.log.append(event.clone())?;
        self.events.push(RecordedEvent { seq, event });
        Ok(seq)
    }

    /// Fails the session terminally with a typed reason.
    pub(crate) fn fail_typed(&mut self, reason: FailureReason) -> RunOutcome {
        if let Ok(failed) = self.status.transition(SessionStatus::Failed) {
            self.status = failed;
        }
        RunOutcome::Failed(reason)
    }
}
