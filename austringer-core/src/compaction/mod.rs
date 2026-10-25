//! Doctrine-compliant compaction (plan todo 13).
//!
//! Eviction (todo 12) reduces the IN-WINDOW projection turn by turn;
//! THIS module is the whole-session doctrine. When usage crosses the
//! user-configured threshold, the provider rejects an over-long
//! context, or the user invokes `/condense`, the engine folds the
//! session into a [`CompactedSession`] that preserves the Anthropic
//! doctrine — architectural decisions, unresolved bugs, ~5 recent
//! files, the last ~10 turns — and emits the T4 hook events and the
//! T11 epoch transition the rest of the austringer consumes.
//!
//! # The overflow-recovery rule (Cline)
//!
//! [`overflow::basic_compact`] is a DETERMINISTIC fallback that never
//! depends on another successful LLM call: a typed
//! [`overflow::ContextOverflow`] input or ANY condenser failure lands
//! on it and still produces a valid compacted session with zero LLM
//! availability (structural selection + head/tail windows + T12
//! folding). Recovery must work when the model does not.
//!
//! # Same-model condensing (Roo)
//!
//! The condense seam ([`condense::SessionCondenser`]) carries the
//! SESSION's model identity; a different-model request is a typed
//! rejection with a documented reason — cheap-model utility roles (the
//! T42 profiles) operate at their OWN sessions and tasks, never by
//! swapping this session's condenser (dialect mismatch).
//!
//! # Externalized segments (Grok) and persisted state (Cline)
//!
//! Every compaction renders recoverable segment files plus an index
//! under `{session}/compaction/` (detail level configurable) through
//! [`segments::SegmentStore`] — auditable and re-loadable — and saves a
//! persisted state validated against a blake3 prefix hash on resume; a
//! mismatch is a typed [`StaleState`] that forces a re-compact.
//!
//! # Never a bare cut
//!
//! Neither the doctrine path nor the fallback naive-truncates:
//! selection always runs ([`doctrine::select`]), and every reduction
//! carries an omission marker or a typed fold.

pub mod condense;
pub mod doctrine;
pub mod engine;
pub mod folded;
pub mod handoff;
pub mod overflow;
pub mod seam;
pub mod segments;
mod state;
pub mod telemetry;

pub use condense::{CondenseRefusal, ModelId, ModelMismatch, SessionCondenser};
pub use doctrine::{DoctrineConfig, DoctrineSummary};
pub use segments::{PersistedCompaction, SegmentDetail, SegmentStore, StaleState};
pub use telemetry::{SealBreach, SealGuard, SealStatus, TelemetryOutcome, TelemetryRecord};

use serde::{Deserialize, Serialize};

use crate::event::{RecordedEvent, Sequence};

/// Rejects streams that are not contiguous and monotonic — compaction
/// never folds a corrupt stream.
pub(crate) fn validate_stream(events: &[RecordedEvent]) -> Result<(), CompactionError> {
    if events.is_empty() {
        return Err(CompactionError::CorruptStream {
            detail: "empty event stream".to_owned(),
        });
    }
    for pair in events.windows(2) {
        if pair[1].seq.0 != pair[0].seq.0 + 1 {
            return Err(CompactionError::CorruptStream {
                detail: format!(
                    "sequence gap or reorder at {} -> {}",
                    pair[0].seq.0, pair[1].seq.0
                ),
            });
        }
    }
    Ok(())
}

/// The user-configurable compaction threshold. `Percent` is the
/// doctrine default (75% of the window); `TokensUsed` and `Remaining`
/// are the absolute bases Zed-style telemetry reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Threshold {
    /// Compact when usage reaches this percent of the window.
    Percent(u64),
    /// Compact when usage reaches this absolute token count.
    TokensUsed(u64),
    /// Compact when this many window tokens remain.
    Remaining(u64),
}

impl Default for Threshold {
    fn default() -> Self {
        Self::Percent(75)
    }
}

impl Threshold {
    /// The basis record when the threshold fires for `used` of
    /// `window` tokens, `None` when it has not fired.
    #[must_use]
    pub fn fired(&self, used: u64, window: u64) -> Option<telemetry::ThresholdBasis> {
        match *self {
            Self::Percent(percent) => (used.saturating_mul(100) >= window.saturating_mul(percent))
                .then_some(telemetry::ThresholdBasis::Percent(percent)),
            Self::TokensUsed(tokens) => {
                (used >= tokens).then_some(telemetry::ThresholdBasis::TokensUsed(used.min(tokens)))
            }
            Self::Remaining(remaining) => (window.saturating_sub(used) <= remaining)
                .then_some(telemetry::ThresholdBasis::Remaining(remaining)),
        }
    }
}

/// The full compaction policy in one typed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionConfig {
    /// What selection preserves (Anthropic doctrine knobs).
    pub doctrine: DoctrineConfig,
    /// When automatic compaction fires.
    pub threshold: Threshold,
    /// A user-supplied condense prompt overriding the default.
    pub custom_condense_prompt: Option<String>,
    /// How much mass segment files carry.
    pub segment_detail: SegmentDetail,
    /// Whether compaction also produces a handoff bundle.
    pub produce_handoff: bool,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            doctrine: DoctrineConfig::default(),
            threshold: Threshold::default(),
            custom_condense_prompt: None,
            segment_detail: SegmentDetail::Standard,
            produce_handoff: true,
        }
    }
}

/// The compacted view of a session: the doctrine summary plus the
/// retained recent window, sealed through `through_seq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactedSession {
    /// The rendered doctrine summary (sections + model condensate when
    /// the doctrine path ran).
    pub summary: String,
    /// The retained recent events (possibly T12-folded tool results).
    pub retained: Vec<RecordedEvent>,
    /// The watermark: the last sequence number folded by this
    /// compaction; everything at or below it is sealed.
    pub through_seq: Sequence,
    /// Token estimate of the whole session before compaction.
    pub before_tokens: u64,
    /// Token estimate of the compacted context after compaction.
    pub after_tokens: u64,
    /// Whether the deterministic fallback produced this session.
    pub fallback_used: bool,
}

impl CompactedSession {
    /// The model-facing compacted context: the summary block plus the
    /// retained history, rendered deterministically. This is what the
    /// next turn's prompt carries instead of the sealed prefix.
    #[must_use]
    pub fn render_context(&self) -> String {
        let mut out = format!(
            "<compacted_context through_seq=\"{}\">\n<summary>\n{}\n</summary>\n",
            self.through_seq.0, self.summary
        );
        out.push_str("<retained_history>\n");
        out.push_str(&crate::context::render_history(&self.retained));
        out.push_str("\n</retained_history>\n</compacted_context>");
        out
    }
}

/// Why a compaction failed, typed.
#[derive(Debug, thiserror::Error)]
pub enum CompactionError {
    /// The event stream is not a contiguous monotonic sequence —
    /// compaction refuses to fold a corrupt stream.
    #[error("corrupt event stream: {detail}")]
    CorruptStream {
        /// What broke (gap, duplicate, out of order, empty).
        detail: String,
    },
    /// The segment store failed (IO, torn, corrupt).
    #[error(transparent)]
    Segment(#[from] segments::SegmentError),
    /// Persisted state does not match the event stream.
    #[error(transparent)]
    Stale(#[from] StaleState),
    /// A hook event could not be appended to the session log.
    #[error("compaction hook append failed: {0}")]
    Hook(String),
}
