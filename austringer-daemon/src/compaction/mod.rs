//! The todo-13 wiring: core's dormant [`CompactionEngine`] as the
//! daemon's pipeline stage set — the threshold-driven auto compaction
//! in the designed `Compaction` slot and the typed `/condense` slash
//! entry — over one shared per-session service.
//!
//! Both paths run the SAME engine: the auto path consults
//! [`SessionCompaction::is_due`] (the [`SignalingCompactor`] seam the
//! hook wrapper expects), the manual path runs unconditionally
//! (`CompactionCause::Manual`). Usage after a compaction is measured
//! from the compacted baseline plus events appended since, so a
//! session that just compacted does not re-fire on its own markers.

mod condenser;
mod stages;

pub use condenser::SharedStack;
pub use stages::{AutoCompactor, CondenseSlash, wire_stages};

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};

use austringer_core::agent_loop::CancelToken;
use austringer_core::agent_loop::stages::{StageCtx, StageError};
use austringer_core::compaction::condense::{DoctrineCondenser, ModelId, SessionCondenser};
use austringer_core::compaction::engine::{
    CompactionCause, CompactionDeps, CompactionEngine, CompactionInput, CompactionSink, HookError,
};
use austringer_core::compaction::segments::SegmentStore;
use austringer_core::compaction::telemetry::ThresholdBasis;
use austringer_core::event::{Event, RecordedEvent, Sequence};
use austringer_core::eviction::{ToolReturnTable, estimate_tokens, events_tokens};

use crate::attach::SessionHost;
use crate::config::CompactionConfig;
use crate::protocol::SessionEvent;

/// The notice kind on the daemon event stream for condense outcomes.
pub const CONDENSE_NOTICE_KIND: &str = "condense";

/// One completed (or refused, or failed) condense run.
pub enum CondenseOutcome {
    /// The engine produced a compacted session.
    Done {
        /// Human-facing outcome notice.
        notice: String,
    },
    /// The run refused before the engine (disabled, empty session).
    Refused {
        /// Why it refused.
        notice: String,
    },
    /// The engine errored; fail-open with the failure surfaced.
    Failed {
        /// The failure notice.
        notice: String,
    },
}

/// The post-compaction usage baseline: what the compacted context
/// carries plus where live events resume.
#[derive(Debug, Clone, Copy, Default)]
struct CompactionState {
    seeded: bool,
    baseline_tokens: u64,
    baseline_seq: u64,
    last_fired_used: u64,
}

/// The per-session compaction service shared by the auto stage and
/// the `/condense` slash entry.
pub struct SessionCompaction {
    engine: CompactionEngine,
    enabled: bool,
    window_tokens: u64,
    model: ModelId,
    condenser: Option<(ModelId, Arc<dyn DoctrineCondenser>)>,
    segments: SegmentStore,
    host: Weak<SessionHost>,
    state: Mutex<CompactionState>,
}

impl SessionCompaction {
    /// Builds the service for one session directory under `config`.
    /// `condenser` carries the same-model registration when the
    /// session's stack is identity-bearing.
    pub fn new(
        session_dir: PathBuf,
        config: &CompactionConfig,
        model: ModelId,
        condenser: Option<(ModelId, Arc<dyn DoctrineCondenser>)>,
        host: Weak<SessionHost>,
    ) -> Self {
        Self {
            engine: CompactionEngine::new(config.engine.clone()),
            enabled: config.enabled,
            window_tokens: config.window_tokens,
            model,
            condenser,
            segments: SegmentStore::new(session_dir),
            host,
            state: Mutex::new(CompactionState::default()),
        }
    }

    /// Whether the configured threshold has fired for the current
    /// usage estimate. Never fires while disabled, on an empty
    /// stream, or before usage has grown past the last compaction.
    #[must_use]
    pub fn is_due(&self, events: &[RecordedEvent]) -> Option<ThresholdBasis> {
        if !self.enabled || events.is_empty() || self.window_tokens == 0 {
            return None;
        }
        let mut state = self.lock_state();
        self.seed_baseline(&mut state, events);
        let used = self.used_from(&state, events);
        if used <= state.last_fired_used {
            return None;
        }
        self.engine.should_compact(used, self.window_tokens)
    }

    /// Runs one compaction through the shared engine. Hook events
    /// (pre-compact marker, condensation, telemetry) append through
    /// the stage context, so the log and the loop mirror stay
    /// coherent; engine errors fail OPEN with the failure surfaced as
    /// a notice (the fail-open-with-notice hooks posture).
    pub async fn run(
        &self,
        ctx: &mut StageCtx<'_>,
        cause: CompactionCause,
    ) -> Result<CondenseOutcome, StageError> {
        if !self.enabled {
            let notice =
                "compaction is disabled in daemon config — /condense did not run".to_owned();
            self.notify(&notice);
            return Ok(CondenseOutcome::Refused { notice });
        }
        let events: Vec<RecordedEvent> = ctx.events().to_vec();
        if events.is_empty() {
            let notice = "nothing to condense: the session has no events yet".to_owned();
            self.notify(&notice);
            return Ok(CondenseOutcome::Refused { notice });
        }
        let used = {
            let mut state = self.lock_state();
            self.seed_baseline(&mut state, &events);
            self.used_from(&state, &events)
        };
        let cancel: &CancelToken = ctx.cancel;
        let tool_returns = ToolReturnTable::default();
        let condenser = self
            .condenser
            .as_ref()
            .map(|(model, inner)| SessionCondenser::for_session(model.clone(), Arc::clone(inner)));
        let mut sink = StageCtxSink { ctx };
        let mut deps = CompactionDeps {
            sink: &mut sink,
            condenser,
            extractor: None,
            segments: &self.segments,
            cancel,
        };
        let input = CompactionInput {
            events: &events,
            tool_returns: &tool_returns,
            model: &self.model,
            used_tokens: used,
            window_tokens: self.window_tokens,
        };
        match self.engine.compact(input, cause, &mut deps).await {
            Ok(result) => {
                let compacted = result.compacted.as_ref();
                let through = compacted.map_or(Sequence(0), |c| c.through_seq);
                if let Some(compacted) = compacted {
                    let mut state = self.lock_state();
                    state.baseline_tokens = compacted.after_tokens;
                    state.baseline_seq = compacted.through_seq.0;
                    state.last_fired_used = used;
                }
                let telemetry = result.telemetry;
                let fallback = if telemetry.fallback_used {
                    " (deterministic fallback)"
                } else {
                    ""
                };
                let notice = format!(
                    "condensed through seq {}: {} -> {} tokens{fallback}",
                    through.0, telemetry.before_tokens, telemetry.after_tokens
                );
                self.notify(&notice);
                Ok(CondenseOutcome::Done { notice })
            }
            Err(error) => {
                let notice = format!("condense failed: {error} — continuing without compaction");
                self.notify(&notice);
                Ok(CondenseOutcome::Failed { notice })
            }
        }
    }

    /// Adopts the newest condensation marker as the usage baseline on
    /// first consultation (a resumed session re-compacts only after
    /// real growth, not on the spot).
    fn seed_baseline(&self, state: &mut CompactionState, events: &[RecordedEvent]) {
        if state.seeded {
            return;
        }
        state.seeded = true;
        if let Some((summary, through_seq)) = last_condensation(events) {
            state.baseline_tokens = estimate_tokens(&summary);
            state.baseline_seq = through_seq;
        }
    }

    /// Current usage: the compacted baseline plus events appended
    /// after it.
    fn used_from(&self, state: &CompactionState, events: &[RecordedEvent]) -> u64 {
        let live_from = events
            .iter()
            .position(|recorded| recorded.seq.0 > state.baseline_seq)
            .unwrap_or(events.len());
        state.baseline_tokens + events_tokens(&events[live_from..])
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, CompactionState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Surfaces the outcome on the daemon event stream (every
    /// frontend sees it; fallback engagement is never silent).
    fn notify(&self, message: &str) {
        if let Some(host) = self.host.upgrade() {
            host.publish(SessionEvent::Notification {
                kind: CONDENSE_NOTICE_KIND.into(),
                message: message.to_owned(),
            });
        }
    }
}

fn last_condensation(events: &[RecordedEvent]) -> Option<(String, u64)> {
    events
        .iter()
        .rev()
        .find_map(|recorded| match &recorded.event {
            Event::Condensation {
                summary,
                through_seq,
            } => Some((summary.clone(), through_seq.0)),
            _ => None,
        })
}

/// The T4 sink over the stage context: engine hook events land in the
/// session log AND the loop mirror (an append-only log writer alone
/// would desynchronize the mirror the next inference reads).
struct StageCtxSink<'a, 'b> {
    ctx: &'a mut StageCtx<'b>,
}

impl CompactionSink for StageCtxSink<'_, '_> {
    fn append(&mut self, event: Event) -> Result<Sequence, HookError> {
        self.ctx
            .append(event)
            .map_err(|error| HookError(error.message))
    }
}
