//! The daemon's Steer stage: drains typed block turns, projection
//! notices, the loop's own submit queue, and the shared mid-run signal
//! queue into the live turn (the todo-2-named extraction from the
//! engine driver module).

use async_trait::async_trait;

use austringer_core::agent_loop::stages::{Flow, StageCtx, StageError, Steer};
use austringer_core::event::{ContentBlock, Event};

use crate::attach::SteerQueue;

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
