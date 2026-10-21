//! The engine driver: a REAL harness-core `AgentLoop` per session —
//! approval through the T7 inspector chain routed to the mediated ask
//! policy (reserved `task_completed` exempted), execution through the
//! daemon's injectable executor factory (worker-backed or
//! registry-backed), mid-run steering through a shared queue the
//! loop's Steer stage drains, and durable turn-workflow journals when
//! a journal root is configured.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use tokio::sync::mpsc;

use harness_core::agent_loop::stages::{Flow, StageCtx, StageError, Steer, ToolExecutor};
use harness_core::agent_loop::{
    AgentLoop, CancelHandle, Inference, LoopConfig, RunOutcome, StageSet,
};
use harness_core::event::{ContentBlock, Event, RecordedEvent};

use crate::attach::{HostCommand, SessionHost, SteerQueue};
use crate::notify::{Notification, NotificationKind, NotificationSink, NotifyError};

/// Pops one drained signal or notice from a shared queue (poison-safe).
fn pop_queued(queue: &SteerQueue) -> Option<String> {
    let mut guard = match queue.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.pop_front()
}

/// A Steer stage draining the daemon's shared queue (mid-run steering:
/// each drained signal becomes a recorded user event inside the live
/// turn).
pub struct SharedSteerer {
    queue: SteerQueue,
    notices: SteerQueue,
}

impl SharedSteerer {
    /// Stage over the shared steer queue and the projection notice
    /// queue.
    #[must_use]
    pub fn new(queue: SteerQueue, notices: SteerQueue) -> Self {
        Self { queue, notices }
    }
}

#[async_trait]
impl Steer for SharedSteerer {
    async fn steer(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        // Typed block turns (protocol v2 media submits) first, then
        // the loop's own submit queue (turn prompts), then any
        // mid-run signals from the daemon's shared queue.
        while let Some(blocks) = ctx.block_queue.pop_front() {
            ctx.append(Event::User { blocks })?;
            *ctx.steered = true;
        }
        // Projection notices (plan M10): one Event::System per media a
        // submit could not project, riding the turn their blocks queued
        // into. LlmVisible by design — the model is told what the
        // marker stands for.
        while let Some(text) = pop_queued(&self.notices) {
            ctx.append(Event::System { text })?;
            *ctx.steered = true;
        }
        while let Some(text) = ctx.steer_queue.pop_front() {
            ctx.append(Event::User {
                blocks: vec![ContentBlock::Text { text }],
            })?;
            *ctx.steered = true;
        }
        loop {
            let next = pop_queued(&self.queue);
            let Some(text) = next else { break };
            ctx.append(Event::User {
                blocks: vec![ContentBlock::Text { text }],
            })?;
            *ctx.steered = true;
        }
        Ok(Flow::Continue)
    }
}

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
        let mut published_through: usize = 0;
        tokio::spawn(async move {
            while let Some(command) = cmd_rx.recv().await {
                match command {
                    HostCommand::Submit(text) => {
                        host.run_turn(&mut engine, &cancel, TurnKickoff::Text(text), &mut published_through)
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
        kickoff: TurnKickoff,
        published_through: &mut usize,
    ) {
        let turn = self.turn_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let mut workflow = self.journal_root.as_ref().and_then(|root| {
            crate::durable::TurnWorkflow::open(root, self.session_id(), turn).ok()
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

    /// The projection notice queue for the steer stage (an empty
    /// dedicated queue when no intake is wired — it drains nothing).
    #[must_use]
    pub fn projection_notice_queue(&self) -> SteerQueue {
        self.projection
            .get()
            .map_or_else(SteerQueue::default, |intake| intake.notice_queue())
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
        stages.approval = Box::new(crate::plan::PlanAwareApproval::new(
            Box::new(crate::stages::ReservedAwareApproval::new(
                crate::stages::chain_over_mediator(self.mediator()),
            )),
            Arc::clone(self),
        ));
        stages.execution = Box::new(crate::plan::PlanGatedExecutor::new(executor, Arc::clone(self)));
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
        stages.approval = Box::new(crate::task::ReadOnlyApproval::new(
            Box::new(crate::stages::ReservedAwareApproval::new(
                crate::stages::chain_over_mediator(self.mediator()),
            )),
            Arc::clone(self),
            agent,
            allowlist,
        ));
        stages.execution = executor;
        stages
    }
}

/// Resolves a session id to its live host (read-only; never creates).
pub type HostResolver = Arc<dyn Fn(&str) -> Option<Arc<SessionHost>> + Send + Sync>;

/// Notification sink mirroring the daemon's scheduled-run kinds onto
/// the owning session's event stream as `SessionEvent::Notification`
/// frames — kind `sched` for a fired automation, `verdict` for a gated
/// completion. Complementary to the `Verdict` frame (todo 1): the same
/// facts on the v1-visible notification lane, beside the engine's own
/// `steer` kind. Other notification kinds keep their existing lanes
/// (asks ride `PermissionAsk`, workers their own frames).
pub struct EventStreamSink {
    resolve: HostResolver,
}

impl EventStreamSink {
    /// Sink resolving sessions through `resolve`.
    #[must_use]
    pub fn new(resolve: HostResolver) -> Self {
        Self { resolve }
    }
}

impl NotificationSink for EventStreamSink {
    fn deliver(&self, notification: &Notification) -> Result<(), NotifyError> {
        let kind = match notification.kind {
            NotificationKind::ScheduledFired => "sched",
            NotificationKind::TurnComplete => "verdict",
            NotificationKind::ApprovalNeeded | NotificationKind::WorkerLost => return Ok(()),
        };
        if let Some(session) = &notification.session_id
            && let Some(host) = (self.resolve)(session)
        {
            host.publish(crate::protocol::SessionEvent::Notification {
                kind: kind.to_owned(),
                message: notification.message.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::mediation::MediationPolicy;
    use crate::protocol::{ServerMsg, SessionEvent};

    #[tokio::test]
    async fn event_stream_sink_maps_only_sched_and_verdict() {
        let (host, _cmd_rx) = SessionHost::new(
            "s-sink",
            64,
            16,
            8,
            MediationPolicy::FirstResponder,
            Duration::from_millis(200),
            None,
        );
        let sink = EventStreamSink::new(Arc::new({
            let host = Arc::clone(&host);
            move |sid: &str| (sid == "s-sink").then(|| Arc::clone(&host))
        }));
        let mut hub = host.hub_subscribe();

        for kind in [
            NotificationKind::ScheduledFired,
            NotificationKind::TurnComplete,
            NotificationKind::ApprovalNeeded,
            NotificationKind::WorkerLost,
        ] {
            sink.deliver(&Notification {
                kind,
                session_id: Some("s-sink".into()),
                message: format!("m-{}", kind.as_str()),
            })
            .expect("deliver never fails");
        }
        // A session-scoped notification for an unknown session is a
        // no-op, not an error.
        sink.deliver(&Notification {
            kind: NotificationKind::ScheduledFired,
            session_id: Some("gone".into()),
            message: "nope".into(),
        })
        .expect("unknown session is a no-op");

        let mut kinds = Vec::new();
        while let Ok(msg) = hub.try_recv() {
            if let ServerMsg::Event { frame } = msg
                && let SessionEvent::Notification { kind, .. } = frame.event
            {
                kinds.push(kind);
            }
        }
        assert_eq!(kinds, vec!["sched".to_owned(), "verdict".to_owned()]);
    }
}
