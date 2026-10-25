//! Stop + compaction hook surfaces (todo 28):
//!
//! - [`HookedStop`] — Stop with block-until semantics driven by prompt
//!   evaluators (or shell/http decisions), bounded by the manager's
//!   LOOP_LIMIT budget: exhaustion is a typed transcript notice and the
//!   exit proceeds. A block re-prompts the model by queueing the reason
//!   as steer input (the pipeline consults StopHook before Inference,
//!   so the queue — not `force_inference` — is what drives the next
//!   inference round). Stop-hook FAILURES are the documented fail-open
//!   exception: always surfaced as transcript notices, never silently
//!   swallowed, and never able to block the exit (the
//!   bounded-iteration guarantee). T5's per-run block cap remains the
//!   structural backstop.
//! - [`HookedCompactor`] — PreCompact/PostCompact around a
//!   [`SignalingCompactor`]. T13's transcript markers (`<pre_compact
//!   …/>` + Condensation) remain the durable record; these hooks are
//!   the user-triggerable surface around actual compactions, never
//!   around idle consultations.

use async_trait::async_trait;
use serde_json::json;

use super::HookEvent;
use super::decision::{HookOpinion, StopAction};
use super::manager::HookOutcome;
use super::shared::SharedHooks;
use crate::agent_loop::stages::{Compactor, Flow, StageCtx, StageError, StopDecision, StopHook};
use crate::event::Event;

/// The real StopHook implementation: iterate-until-goal, bounded.
pub struct HookedStop {
    /// Shared hook state.
    pub manager: SharedHooks,
}

#[async_trait]
impl StopHook for HookedStop {
    async fn on_stop(&mut self, ctx: &mut StageCtx<'_>) -> Result<StopDecision, StageError> {
        let payload = json!({"event": "stop", "turn": ctx.turn});
        let mut manager = self.manager.inner().lock().await;
        let fired = manager.fire(HookEvent::Stop, payload).await;

        let mut block = None;
        let mut failures = Vec::new();
        for firing in &fired {
            match &firing.outcome {
                HookOutcome::Succeeded(HookOpinion::Stop(StopAction::Block { reason }))
                    if block.is_none() =>
                {
                    block = Some(reason.clone());
                }
                HookOutcome::Failed(error) => {
                    failures.push(format!("{}: {error}", firing.hook));
                }
                HookOutcome::Succeeded(_)
                | HookOutcome::InertPendingReview
                | HookOutcome::InertModified => {}
            }
        }

        let decision = match block {
            Some(reason) => {
                if manager.stop_budget_is_exhausted() {
                    let notice = format!(
                        "stop-hook loop budget exhausted ({} iterations); letting exit proceed",
                        manager.stop_loop_limit()
                    );
                    ctx.append(Event::System { text: notice })?;
                    StopDecision::Proceed
                } else {
                    manager.consume_stop_iteration();
                    ctx.steer_queue.push_back(format!("[stop-hook] {reason}"));
                    StopDecision::Block(reason)
                }
            }
            None => StopDecision::Proceed,
        };

        // Except-Stop-semantics: failures never block the exit, but they
        // are NEVER silent — each lands in the transcript.
        for failure in failures {
            let notice =
                format!("stop hook failed: {failure} — proceeding (bounded-iteration guarantee)");
            ctx.append(Event::System { text: notice })?;
        }
        Ok(decision)
    }
}

/// A compaction source that can say whether a compaction is due. The
/// pipeline's `Compactor` trait cannot express this, and firing
/// Pre/PostCompact hooks around idle consultations would lie; the T13
/// engine adapter (which owns `should_compact`) implements this when
/// wired into the loop.
#[async_trait]
pub trait SignalingCompactor: Send {
    /// Whether a compaction should run now.
    async fn is_due(&mut self, ctx: &mut StageCtx<'_>) -> Result<bool, StageError>;

    /// Runs the compaction (only consulted after `is_due` returned
    /// true).
    async fn run_compaction(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError>;
}

/// A [`SignalingCompactor`] wrapped by PreCompact/PostCompact hooks:
/// they fire around actual compactions only.
pub struct HookedCompactor {
    /// Shared hook state.
    pub manager: SharedHooks,
    /// The wrapped signaling compaction source.
    pub inner: Box<dyn SignalingCompactor>,
}

#[async_trait]
impl Compactor for HookedCompactor {
    async fn compact(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        if !self.inner.is_due(ctx).await? {
            return Ok(Flow::Continue);
        }
        {
            let mut manager = self.manager.inner().lock().await;
            manager
                .fire(HookEvent::PreCompact, json!({"event": "pre_compact"}))
                .await;
        }
        let flow = self.inner.run_compaction(ctx).await;
        {
            let mut manager = self.manager.inner().lock().await;
            manager
                .fire(HookEvent::PostCompact, json!({"event": "post_compact"}))
                .await;
        }
        flow
    }
}
