//! The engine driver: a REAL austringer-core `AgentLoop` per session —
//! approval through the T7 inspector chain routed to the mediated ask
//! policy (reserved `task_completed` exempted), execution through the
//! daemon's injectable executor factory (worker-backed or
//! registry-backed), mid-run steering through a shared queue the
//! loop's Steer stage drains, and durable turn-workflow journals when
//! a journal root is configured.
//!
//! Layout: the steer stage lives in [`steerer`], the notification sink
//! and host resolver in [`sink`]; this module owns the driver (spawn,
//! the turn loop, the sessions-store live feed, and the stage-set
//! builders).

mod sink;
mod steerer;

pub use sink::{EventStreamSink, HostResolver};
pub use steerer::SharedSteerer;

/// The session system prompt riding every inference request (the
/// stable-first prefix, research §24.6). Frozen by construction: no
/// timestamps, paths, or session state ever enter it — a byte change
/// here invalidates every session's provider prefix cache.
pub const SESSION_SYSTEM_PROMPT: &str = "You are austringer, a local-first coding agent. \
Work inside the user's repository through the declared tools, respect approval gates and \
budgets, prefer the smallest correct change, and report outcomes concretely.";

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::mpsc;

use austringer_core::agent_loop::stages::ToolExecutor;
use austringer_core::agent_loop::{
    AgentLoop, CancelHandle, Inference, LoopConfig, RunOutcome, StageSet,
};
use austringer_core::event::{ContentBlock, Event, RecordedEvent};

use crate::attach::{HostCommand, SessionHost, SteerQueue};

/// How a turn starts: plain text or typed blocks (v2 media submits).
enum TurnKickoff {
    /// A text prompt.
    Text(String),
    /// Typed content blocks.
    Blocks(Vec<ContentBlock>),
}

impl TurnKickoff {
    fn journal_text(&self) -> String {
        match self {
            TurnKickoff::Text(text) => text.clone(),
            TurnKickoff::Blocks(blocks) => blocks
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text.clone(),
                    other => other.text_marker(),
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
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
        // Resume-safe start: events replayed by `AgentLoop::open` were
        // already published (and persisted) by the previous host —
        // publishing resumes AFTER them, or an evicted-and-recreated
        // session would re-broadcast its entire history to the hub and
        // re-append it to the sessions store on the next turn.
        let mut published_through: usize = engine.events().len();
        tokio::spawn(async move {
            while let Some(command) = cmd_rx.recv().await {
                match command {
                    HostCommand::Submit(text) => {
                        host.run_turn(
                            &mut engine,
                            &cancel,
                            TurnKickoff::Text(text),
                            &mut published_through,
                        )
                        .await;
                    }
                    HostCommand::SubmitMedia { .. } => {
                        // Intake resolves SubmitMedia to SubmitBlocks
                        // before queueing; the driver never sees the
                        // unresolved form.
                    }
                    HostCommand::SubmitBlocks(blocks) => {
                        host.run_turn(
                            &mut engine,
                            &cancel,
                            TurnKickoff::Blocks(blocks),
                            &mut published_through,
                        )
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
            // Eviction closed the lane: the engine (and its transcript
            // mirror) drops with this task's future.
            host.mark_engine_task_finished();
        });
        Ok(())
    }

    /// Journal a delivered signal on the current turn's workflow and
    /// surface it on the event stream.
    pub(crate) fn record_signal(&self, text: &str) {
        if let Some(root) = &self.journal_root
            && let Ok(mut journal) = crate::journal::Journal::open_capped(
                &crate::journal::turn_journal_path(
                    root,
                    self.session_id(),
                    self.turn_counter.load(Ordering::SeqCst).max(1),
                ),
                self.journal_max_bytes(),
            )
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
        kickoff: TurnKickoff,
        published_through: &mut usize,
    ) {
        // RAII: an in-flight turn blocks idle eviction (and a panicked
        // engine still unwinds the flag).
        let _in_flight = crate::attach::InFlightTurn::mark(self);
        self.mark_activity();
        let turn = self.turn_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let mut workflow = self.journal_root.as_ref().and_then(|root| {
            crate::durable::TurnWorkflow::open_capped(
                root,
                self.session_id(),
                turn,
                self.journal_max_bytes(),
            )
            .ok()
        });
        if let Some(workflow) = workflow.as_mut() {
            let _ = workflow.begin(&kickoff.journal_text());
        }
        self.publish(crate::protocol::SessionEvent::TurnStarted { turn });
        match kickoff {
            TurnKickoff::Text(text) => engine.submit(text),
            TurnKickoff::Blocks(blocks) => engine.submit_blocks(blocks),
        }
        let outcome = engine.run().await;
        let outcome_text = match &outcome {
            RunOutcome::TaskCompleted => "task_completed".to_owned(),
            RunOutcome::TurnComplete => "turn_complete".to_owned(),
            RunOutcome::Cancelled(record) => format!("cancelled@{}", record.high_water.0),
            RunOutcome::Failed(reason) => format!("failed::{reason:?}"),
        };
        let new_events: Vec<RecordedEvent> = engine.events()[*published_through..].to_vec();
        *published_through = engine.events().len();
        // Durable before broadcast: the store append precedes the
        // Recorded frames, so a client that saw the events can find
        // them through `sessions search` immediately.
        self.persist_session_events(&new_events);
        for recorded in &new_events {
            self.publish(crate::protocol::SessionEvent::Recorded {
                seq: recorded.seq.0,
                event: recorded.event.clone(),
            });
        }
        // Checkpoint recording wraps the pipeline's OUTSIDE: the turn
        // is done and tools are quiet (the crate's safe boundary), and
        // TurnFinished is held until the checkpoint lands so a client
        // that saw the finish can /rewind immediately. Fail-open with
        // notice — a recording failure never kills the turn.
        self.record_turn_checkpoint(turn, &new_events).await;
        self.publish(crate::protocol::SessionEvent::TurnFinished {
            turn,
            outcome: outcome_text.clone(),
        });
        if let Some(workflow) = workflow.as_mut() {
            let _ = workflow.finish(&outcome_text);
        }
    }

    /// The projection notice queue for the steer stage (an empty
    /// dedicated queue when no intake is wired — it drains nothing).
    #[must_use]
    pub fn projection_notice_queue(&self) -> SteerQueue {
        self.projection
            .get()
            .map_or_else(SteerQueue::default, |intake| intake.notice_queue())
    }

    /// The sessions-store live feed: appends the turn's recorded events
    /// (event rows + FTS) so `sessions search` covers live sessions; the
    /// first append creates the session row. Hosts built outside the
    /// session factory have no store root — honest absence. Best-effort
    /// by design: a store failure degrades the turn's search coverage,
    /// never the conversation (surfaced as a warn log, the fail-open
    /// transcript-notice posture would need a wire kind per surface).
    fn persist_session_events(&self, recorded: &[RecordedEvent]) {
        use austringer_sessions::{LeaseOwner, SessionStore};
        let Some(root) = self.sessions_store_root.get() else {
            return;
        };
        if recorded.is_empty() {
            return;
        }
        let events: Vec<Event> = recorded.iter().map(|r| r.event.clone()).collect();
        let id = self.session_id();
        let result = (|| {
            let store = SessionStore::open(root)?;
            let owner = LeaseOwner::new(id);
            let handle = match store.open_session_rw(id, &owner) {
                Ok(handle) => handle,
                Err(austringer_sessions::SessionError::NotFound { .. }) => {
                    store.create(id, "")?;
                    store.open_session_rw(id, &owner)?
                }
                Err(error) => return Err(error),
            };
            handle.append_batch(&events).map(|_| ())
        })();
        if let Err(error) = result {
            tracing::warn!(session = %id, %error, "sessions store live feed degraded; this turn's search coverage is stale");
        }
    }

    /// Build the daemon's standard stage set over a tool-execution
    /// stage (worker-backed or registry-backed; the session factory's
    /// injected executor decides). The approval and execution stages
    /// carry the todo-16 plan gate wrappers: inactive by default
    /// (byte-identical default path, pinned by the golden-transcript
    /// fixture), they block write tools both pre-approval and again at
    /// execution time while a plan is pending.
    #[must_use]
    pub fn standard_stages(self: &Arc<Self>, executor: Box<dyn ToolExecutor>) -> StageSet {
        let mut stages = StageSet::default();
        stages.steer = Box::new(SharedSteerer::new(
            self.steer_queue(),
            self.projection_notice_queue(),
        ));
        stages.exit_on_error = Box::new(crate::cost_ledger::CostStopExit::new(Box::new(
            austringer_core::agent_loop::stages::defaults::FailFast,
        )));
        stages.approval = Box::new(crate::plan::PlanAwareApproval::new(
            Box::new(crate::stages::ReservedAwareApproval::new(
                crate::stages::chain_over_mediator(self.mediator()),
            )),
            Arc::clone(self),
        ));
        stages.execution = Box::new(crate::plan::PlanGatedExecutor::new(
            executor,
            Arc::clone(self),
        ));
        crate::compaction::wire_stages(&mut stages, self.compaction.get().cloned());
        stages
    }

    /// The todo-14 variant for curated read-only agent children:
    /// steer and execution identical to [`Self::standard_stages`];
    /// the approval chain is wrapped in
    /// [`crate::task::ReadOnlyApproval`] so every out-of-allowlist
    /// call is refused before any ask fires and logged on the child
    /// stream.
    #[must_use]
    pub fn readonly_stages(
        self: &Arc<Self>,
        executor: Box<dyn ToolExecutor>,
        agent: &str,
        allowlist: Vec<String>,
    ) -> StageSet {
        let mut stages = StageSet::default();
        stages.steer = Box::new(SharedSteerer::new(
            self.steer_queue(),
            self.projection_notice_queue(),
        ));
        stages.exit_on_error = Box::new(crate::cost_ledger::CostStopExit::new(Box::new(
            austringer_core::agent_loop::stages::defaults::FailFast,
        )));
        stages.approval = Box::new(crate::task::ReadOnlyApproval::new(
            Box::new(crate::stages::ReservedAwareApproval::new(
                crate::stages::chain_over_mediator(self.mediator()),
            )),
            Arc::clone(self),
            agent,
            allowlist,
        ));
        stages.execution = executor;
        crate::compaction::wire_stages(&mut stages, self.compaction.get().cloned());
        stages
    }
}
