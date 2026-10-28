//! The stage adapters over [`SessionCompaction`]: the auto path fills
//! the pipeline's designed `Compaction` slot (and implements the
//! [`SignalingCompactor`] seam the hook wrapper expects), the manual
//! path is the `/condense` slash entry.

use std::sync::Arc;

use async_trait::async_trait;

use austringer_core::agent_loop::stages::{
    Compactor, Flow, SlashCommands, SlashFlow, StageCtx, StageError, StageSet,
};
use austringer_core::compaction::engine::CompactionCause;
use austringer_core::event::Event;
use austringer_core::hooks::stop::SignalingCompactor;

use super::{CondenseOutcome, SessionCompaction};

/// The auto-compaction stage: consults the threshold, runs the shared
/// engine when it fires. Failures fail OPEN with a transcript notice
/// (the engine already appended its failed-telemetry marker).
pub struct AutoCompactor {
    service: Arc<SessionCompaction>,
}

impl AutoCompactor {
    /// Wraps the shared service.
    pub fn new(service: Arc<SessionCompaction>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl SignalingCompactor for AutoCompactor {
    async fn is_due(&mut self, ctx: &mut StageCtx<'_>) -> Result<bool, StageError> {
        Ok(self.service.is_due(ctx.events()).is_some())
    }

    async fn run_compaction(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        match self.service.run(ctx, CompactionCause::Auto).await? {
            CondenseOutcome::Done { .. } => Ok(Flow::Continue),
            CondenseOutcome::Refused { notice } | CondenseOutcome::Failed { notice } => {
                ctx.append(Event::System { text: notice })?;
                Ok(Flow::Continue)
            }
        }
    }
}

#[async_trait]
impl Compactor for AutoCompactor {
    async fn compact(&mut self, ctx: &mut StageCtx<'_>) -> Result<Flow, StageError> {
        if SignalingCompactor::is_due(self, ctx).await? {
            SignalingCompactor::run_compaction(self, ctx).await
        } else {
            Ok(Flow::Continue)
        }
    }
}

/// The `/condense` slash entry: runs the same engine manually
/// (below threshold too). Any other command stays `Unknown` — the
/// loop's default unknown-command notice is byte-identical to the
/// stock stage's.
pub struct CondenseSlash {
    service: Arc<SessionCompaction>,
}

impl CondenseSlash {
    /// Wraps the shared service.
    pub fn new(service: Arc<SessionCompaction>) -> Self {
        Self { service }
    }
}

/// Fills the designed Compaction and SlashCommand stage slots from
/// the factory-bound service (todo 13); `None` (hosts built outside
/// the factory) keeps the stock defaults — honest absence.
pub fn wire_stages(stages: &mut StageSet, service: Option<Arc<SessionCompaction>>) {
    let Some(service) = service else { return };
    stages.compaction = Box::new(AutoCompactor::new(Arc::clone(&service)));
    stages.slash_commands = Box::new(CondenseSlash::new(service));
}

#[async_trait]
impl SlashCommands for CondenseSlash {
    async fn dispatch(
        &mut self,
        ctx: &mut StageCtx<'_>,
        command: &str,
    ) -> Result<SlashFlow, StageError> {
        match command.strip_prefix("/condense") {
            Some(rest) if rest.is_empty() || rest.starts_with(' ') => {}
            _ => return Ok(SlashFlow::Unknown),
        }
        let notice = match self.service.run(ctx, CompactionCause::Manual).await? {
            CondenseOutcome::Done { notice }
            | CondenseOutcome::Refused { notice }
            | CondenseOutcome::Failed { notice } => notice,
        };
        Ok(SlashFlow::Handled {
            notice: Some(notice),
        })
    }
}
