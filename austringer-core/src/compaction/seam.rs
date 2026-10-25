//! The run boundary of the compaction engine (plan todo 13): the
//! input, the cause, the event-log sink, the injected deps, and the
//! completed-run result. These are the types callers wire; the
//! orchestration lives in [`super::engine`].

use std::sync::Arc;

use super::CompactedSession;
use super::condense::{ModelId, SessionCondenser};
use super::handoff::{HandoffBundle, MilestoneMarker, ThreadExtractor};
use super::overflow::ContextOverflow;
use super::segments::SegmentStore;
use super::telemetry::{SealGuard, TelemetryRecord};
use crate::agent_loop::CancelToken;
use crate::context::EpochTransition;
use crate::event::{Event, RecordedEvent, Sequence};
use crate::eviction::ToolReturnTable;
use crate::log::{AppendError, EventLogWriter};

/// Everything one compaction consumes.
pub struct CompactionInput<'a> {
    /// The full LLM-visible session stream (contiguous from its first
    /// sequence).
    pub events: &'a [RecordedEvent],
    /// Authoritative return codes for every executed call.
    pub tool_returns: &'a ToolReturnTable,
    /// The session's model identity (same-model condensing).
    pub model: &'a ModelId,
    /// Current whole-context usage estimate.
    pub used_tokens: u64,
    /// The model's context window.
    pub window_tokens: u64,
}

/// Why the compaction runs. `Manual` is the typed `/condense` entry
/// (the command layer wires it later); `Overflow` is the provider's
/// rejection of an over-long context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionCause {
    /// The user asked for it.
    Manual,
    /// The configured threshold fired.
    Auto,
    /// The provider rejected an over-long context.
    Overflow(ContextOverflow),
}

/// Why a hook append failed.
#[derive(Debug, thiserror::Error)]
#[error("hook append failed: {0}")]
pub struct HookError(pub String);

/// The T4 event-log seam the engine appends hook events through.
pub trait CompactionSink {
    /// Appends one event, assigning its sequence.
    fn append(&mut self, event: Event) -> Result<Sequence, HookError>;
}

impl CompactionSink for EventLogWriter {
    fn append(&mut self, event: Event) -> Result<Sequence, HookError> {
        self.append(event)
            .map_err(|error: AppendError| HookError(error.to_string()))
    }
}

/// The injected seams one compaction run may call.
pub struct CompactionDeps<'a> {
    /// The session event log (hook events).
    pub sink: &'a mut dyn CompactionSink,
    /// The same-model condenser, when the doctrine path may run.
    pub condenser: Option<SessionCondenser>,
    /// The second-model extractor for handoff distillation.
    pub extractor: Option<Arc<dyn ThreadExtractor>>,
    /// The externalized segment store.
    pub segments: &'a SegmentStore,
    /// Cooperative cancellation.
    pub cancel: &'a CancelToken,
}

/// The completed run: what was produced and what the loop wires
/// forward.
#[derive(Debug)]
pub struct CompactionResult {
    /// The compacted session (`None` on cancel).
    pub compacted: Option<CompactedSession>,
    /// The telemetry record (always present).
    pub telemetry: TelemetryRecord,
    /// The seal guard for postcompact accountability.
    pub seal_guard: Option<SealGuard>,
    /// The handoff bundle, when configured.
    pub handoff: Option<HandoffBundle>,
    /// The latest milestone in the stream, when any.
    pub milestone: Option<MilestoneMarker>,
    /// Segment files written.
    pub segments: Vec<std::path::PathBuf>,
    /// The T11 transition for the epoch ledger.
    pub epoch_transition: Option<EpochTransition>,
}

impl CompactionResult {
    /// The canceled run: telemetry present, nothing produced, no
    /// artifacts retained.
    pub(super) fn canceled(telemetry: TelemetryRecord) -> Self {
        Self {
            compacted: None,
            telemetry,
            seal_guard: None,
            handoff: None,
            milestone: None,
            segments: Vec::new(),
            epoch_transition: None,
        }
    }
}
