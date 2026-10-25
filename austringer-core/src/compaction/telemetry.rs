//! Compaction telemetry and postcompact accountability (plan todo 13,
//! Zed + forge patterns).
//!
//! Every compaction appends a [`TelemetryRecord`] to the event log as a
//! parseable `<compaction_telemetry …/>` system marker: the outcome
//! (manual/auto/canceled/failed), the threshold basis that fired
//! (percent | tokens-used | remaining), before/after token counts, and
//! the seal-integrity status. The seal itself is a first-class object:
//! after a compaction, [`SealGuard`] watches the live stream, and any
//! post-compact write into the sealed span surfaces as a pageable
//! [`SealBreach`] event carrying the offending sequence range — never
//! silently absorbed.

use crate::event::{Event, RecordedEvent, Sequence};

/// How a compaction ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryOutcome {
    /// The user invoked `/condense` (typed manual entry).
    Manual,
    /// The threshold fired automatically.
    Auto,
    /// Cancellation landed mid-compaction (no artifacts).
    Canceled,
    /// The compaction errored after the pre-compact hook.
    Failed,
}

impl TelemetryOutcome {
    /// The marker name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::Canceled => "canceled",
            Self::Failed => "failed",
        }
    }
}

/// The threshold basis a compaction fired under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdBasis {
    /// Percent of the context window.
    Percent(u64),
    /// Absolute tokens used.
    TokensUsed(u64),
    /// Tokens remaining in the window.
    Remaining(u64),
}

impl ThresholdBasis {
    /// The marker payload.
    #[must_use]
    pub fn render(&self) -> String {
        match *self {
            Self::Percent(value) => format!("percent:{value}"),
            Self::TokensUsed(value) => format!("tokens-used:{value}"),
            Self::Remaining(value) => format!("remaining:{value}"),
        }
    }
}

/// Seal integrity of a compacted session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealStatus {
    /// No post-compact write touched the sealed span.
    Intact,
    /// A write landed inside the sealed span.
    Breached,
}

/// One compaction's accountability record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryRecord {
    /// The compaction ordinal (segment-store identity).
    pub compaction_id: u64,
    /// How the compaction ended.
    pub outcome: TelemetryOutcome,
    /// The threshold basis that fired.
    pub basis: ThresholdBasis,
    /// Session tokens before compaction.
    pub before_tokens: u64,
    /// Compacted-context tokens after.
    pub after_tokens: u64,
    /// Whether the deterministic fallback produced the session.
    pub fallback_used: bool,
    /// Seal status at record time.
    pub seal: SealStatus,
}

impl TelemetryRecord {
    /// The parseable system-marker form appended to the event log.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "<compaction_telemetry id=\"{}\" outcome=\"{}\" basis=\"{}\" \
             before=\"{}\" after=\"{}\" fallback=\"{}\" seal=\"{}\"/>",
            self.compaction_id,
            self.outcome.name(),
            self.basis.render(),
            self.before_tokens,
            self.after_tokens,
            self.fallback_used,
            match self.seal {
                SealStatus::Intact => "intact",
                SealStatus::Breached => "breached",
            },
        )
    }

    /// The typed system event carrying this record.
    #[must_use]
    pub fn event(&self) -> Event {
        Event::System {
            text: self.render(),
        }
    }
}

/// A breached seal: post-compact writes landed inside the sealed span.
/// First-class and pageable — the marker names the watermark and the
/// offending sequence range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealBreach {
    /// The compaction whose seal was breached.
    pub compaction_id: u64,
    /// The watermark the seal covers (writes at or below it are
    /// sealed).
    pub watermark: Sequence,
    /// The inclusive `(first, last)` offending sequence range.
    pub offending: (Sequence, Sequence),
}

impl SealBreach {
    /// The parseable pageable marker form.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "<seal_breach compaction=\"{}\" watermark=\"{}\" \
             offending=\"{}..{}\"/>",
            self.compaction_id, self.watermark.0, self.offending.0.0, self.offending.1.0
        )
    }

    /// The typed system event carrying this breach.
    #[must_use]
    pub fn event(&self) -> Event {
        Event::System {
            text: self.render(),
        }
    }
}

/// Watches the live stream after a compaction: the compacted prefix
/// `[1..=watermark]` is sealed; the log is append-only, so any
/// post-seal event at or below the watermark is a rewrite of sealed
/// history — a breach. Appends above the watermark are normal life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealGuard {
    compaction_id: u64,
    watermark: Sequence,
    status: SealStatus,
}

impl SealGuard {
    /// A fresh guard for the compaction that sealed through
    /// `watermark`.
    #[must_use]
    pub const fn new(compaction_id: u64, watermark: Sequence) -> Self {
        Self {
            compaction_id,
            watermark,
            status: SealStatus::Intact,
        }
    }

    /// The sealed watermark.
    #[must_use]
    pub const fn watermark(&self) -> Sequence {
        self.watermark
    }

    /// Current integrity status.
    #[must_use]
    pub const fn status(&self) -> SealStatus {
        self.status
    }

    /// Observes post-compact events; returns the breach (at most one
    /// per call) when any event writes into the sealed span.
    pub fn observe(&mut self, events: &[RecordedEvent]) -> Vec<SealBreach> {
        let offenders: Vec<Sequence> = events
            .iter()
            .map(|recorded| recorded.seq)
            .filter(|seq| seq.0 <= self.watermark.0)
            .collect();
        let Some(&first) = offenders.first() else {
            return Vec::new();
        };
        self.status = SealStatus::Breached;
        let last = offenders.iter().copied().max().unwrap_or(first);
        vec![SealBreach {
            compaction_id: self.compaction_id,
            watermark: self.watermark,
            offending: (first, last),
        }]
    }
}
