//! The engine driver: a REAL harness-core `AgentLoop` per session —
//! approval through the T7 inspector chain routed to the mediated ask
//! policy (reserved `task_completed` exempted), execution through the
//! split-brain worker stage, mid-run steering through a shared queue
//! the loop's Steer stage drains, and durable turn-workflow journals
//! when a journal root is configured.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use tokio::sync::mpsc;

use harness_core::agent_loop::stages::{Flow, StageCtx, StageError, Steer};
use harness_core::agent_loop::{
    AgentLoop, CancelHandle, Inference, LoopConfig, RunOutcome, StageSet,
};
use harness_core::event::{ContentBlock, Event, RecordedEvent};

use crate::attach::{HostCommand, SessionHost, SteerQueue};
use crate::stages::WorkerToolExecutor;

/// A Steer stage draining the daemon's shared queue (mid-run steering:
/// each drained signal becomes a recorded user event inside the live
/// turn).
pub struct SharedSteerer {
    queue: SteerQueue,
}

impl SharedSteerer {
    /// Stage over the shared queue.
    #[must_use]
    pub fn new(queue: SteerQueue) -> Self {
        Self { queue }
    }
}

#[async_trait]
impl Steer for SharedSteerer {
    async fn steer(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        // Drain the loop's own submit queue first (turn prompts), then
        // any mid-run signals from the daemon's shared queue.
        while let Some(text) = ctx.steer_queue.pop_front() {
            ctx.append(Event::User {
                blocks: vec![ContentBlock::Text { text }],
            })?;
            *ctx.steered = true;
        }
        loop {
            let next = {
                let mut guard = match self.queue.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard.pop_front()
            };
            let Some(text) = next else { break };
            ctx.append(Event::User {
                blocks: vec![ContentBlock::Text { text }],
            })?;
            *ctx.steered = true;
        }
        Ok(Flow::Continue)
    }
}

impl SessionHost {
    /// Spawn the engine driver with default loop configuration.
    pub fn spawn_engine(
        self: &Arc<Self>,
        session_dir: PathBuf,
        stages: StageSet,
        inference: Box<dyn Inference>,
        cmd_rx: mpsc::Receiver<HostCommand>,
    ) -> Result<(), std::io::Error> {
        self.spawn_engine_with_config(
            session_dir,
            stages,
            inference,
            cmd_rx,
            LoopConfig::default(),
        )
    }

    /// Spawn the engine driver: owns the `AgentLoop`, consumes host
    /// commands, publishes recorded events after each run, journals
    /// turn workflows when a journal root is configured. Scheduled
    /// runs pass a per-automation turn budget via `loop_config`.
    pub fn spawn_engine_with_config(
        self: &Arc<Self>,
        session_dir: PathBuf,
        stages: StageSet,
        inference: Box<dyn Inference>,
        mut cmd_rx: mpsc::Receiver<HostCommand>,
        loop_config: LoopConfig,
    ) -> Result<(), std::io::Error> {
        let mut engine = AgentLoop::open(&session_dir, loop_config, stages, inference)?;
        let host = Arc::clone(self);
        let cancel = engine.cancel_handle();
        let mut published_through: usize = 0;
        tokio::spawn(async move {
            while let Some(command) = cmd_rx.recv().await {
                match command {
                    HostCommand::Submit(text) => {
                        host.run_turn(&mut engine, &cancel, text, &mut published_through)
                            .await;
                    }
                    HostCommand::Steer(text) => {
                        // Belt-and-braces: the intake path normally
                        // handles Steer mid-run; a Steer queued here is
                        // absorbed at the next iteration regardless.
                        host.record_signal(&text);
                        if let Ok(mut queue) = host.steer_queue().lock() {
                            queue.push_back(text);
                        }
                    }
                    HostCommand::Cancel => cancel.cancel(),
                }
            }
        });
        Ok(())
    }

    /// Journal a delivered signal on the current turn's workflow and
    /// surface it on the event stream.
    pub(crate) fn record_signal(&self, text: &str) {
        if let Some(root) = &self.journal_root
            && let Ok(mut journal) =
                crate::journal::Journal::open(&crate::journal::turn_journal_path(
                    root,
                    self.session_id(),
                    self.turn_counter.load(Ordering::SeqCst).max(1),
                ))
        {
            let _ = journal.record(crate::journal::JournalStep::Signal {
                text: text.to_owned(),
            });
        }
        self.publish(crate::protocol::SessionEvent::Notification {
            kind: "steer".into(),
            message: format!("signal queued: {text}"),
        });
    }

    async fn run_turn(
        &self,
        engine: &mut AgentLoop,
        _cancel: &CancelHandle,
        prompt: String,
        published_through: &mut usize,
    ) {
        let turn = self.turn_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let mut workflow = self.journal_root.as_ref().and_then(|root| {
            crate::durable::TurnWorkflow::open(root, self.session_id(), turn).ok()
        });
        if let Some(workflow) = workflow.as_mut() {
            let _ = workflow.begin(&prompt);
        }
        self.publish(crate::protocol::SessionEvent::TurnStarted { turn });
        engine.submit(prompt);
        let outcome = engine.run().await;
        let outcome_text = match &outcome {
            RunOutcome::TaskCompleted => "task_completed".to_owned(),
            RunOutcome::TurnComplete => "turn_complete".to_owned(),
            RunOutcome::Cancelled(record) => format!("cancelled@{}", record.high_water.0),
            RunOutcome::Failed(reason) => format!("failed::{reason:?}"),
        };
        let new_events: Vec<RecordedEvent> = engine.events()[*published_through..].to_vec();
        *published_through = engine.events().len();
        for recorded in new_events {
            self.publish(crate::protocol::SessionEvent::Recorded {
                seq: recorded.seq.0,
                event: recorded.event,
            });
        }
        self.publish(crate::protocol::SessionEvent::TurnFinished {
            turn,
            outcome: outcome_text.clone(),
        });
        if let Some(workflow) = workflow.as_mut() {
            let _ = workflow.finish(&outcome_text);
        }
    }

    /// Build the daemon's standard stage set over a worker executor.
    #[must_use]
    pub fn standard_stages(&self, executor: WorkerToolExecutor) -> StageSet {
        let mut stages = StageSet::default();
        stages.steer = Box::new(SharedSteerer::new(self.steer_queue()));
        stages.approval = Box::new(crate::stages::ReservedAwareApproval::new(
            crate::stages::chain_over_mediator(self.mediator()),
        ));
        stages.execution = Box::new(executor);
        stages
    }
}
