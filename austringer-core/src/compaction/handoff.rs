//! The handoff complement and the teamwork successor-handoff (plan
//! todo 13, Amp + Antigravity patterns).
//!
//! A [`HandoffBundle`] is a model-distilled transfer of built-up
//! context into a FRESH session for long-horizon work — it complements
//! compaction, never replaces it. It is produced at compaction time
//! (deterministically from the compacted session, or through an
//! injected second-model extractor), and [`read_thread`] performs the
//! read_thread-style extraction over REFERENCED sessions through that
//! same injected seam. At milestone boundaries,
//! [`SuccessorBundle::at_milestone`] packages the handoff plus the
//! focus-chain state and open todos so the daemon can spin a fresh
//! successor session instead of compaction-aging one long context.

use async_trait::async_trait;

use super::CompactedSession;
use crate::event::{Event, RecordedEvent, Sequence};
use crate::eviction::CondenseError;

/// A pointer into the originating session the handoff references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SalientPointer {
    /// The sequence number pointed at.
    pub seq: Sequence,
    /// What lives there.
    pub note: String,
}

/// The distilled transfer package for a fresh session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffBundle {
    /// The distilled summary (doctrine render or second-model output).
    pub distilled_summary: String,
    /// Pointers into the originating session.
    pub salient_pointers: Vec<SalientPointer>,
}

impl HandoffBundle {
    /// The deterministic at-compaction handoff: the compacted summary
    /// IS the distilled transfer; pointers reference the sealed span
    /// boundary and the retained window.
    #[must_use]
    pub fn from_compaction(compacted: &CompactedSession) -> Self {
        let mut pointers = vec![SalientPointer {
            seq: compacted.through_seq,
            note: "compaction watermark: everything above survives verbatim".to_owned(),
        }];
        if let Some(first) = compacted.retained.first() {
            pointers.push(SalientPointer {
                seq: first.seq,
                note: "retained window start".to_owned(),
            });
        }
        Self {
            distilled_summary: compacted.summary.clone(),
            salient_pointers: pointers,
        }
    }
}

/// The injected second-model seam extracting a handoff from a
/// referenced session (tests inject a mock; the daemon wires a real
/// provider).
#[async_trait]
pub trait ThreadExtractor: Send + Sync {
    /// Distills `events` into a handoff bundle.
    async fn extract_thread(
        &self,
        events: &[RecordedEvent],
    ) -> Result<HandoffBundle, CondenseError>;
}

/// read_thread-style extraction over a referenced session through the
/// injected extractor seam.
pub async fn read_thread(
    extractor: &dyn ThreadExtractor,
    events: &[RecordedEvent],
) -> Result<HandoffBundle, CondenseError> {
    extractor.extract_thread(events).await
}

/// A typed milestone marker riding system events as a parseable
/// `<milestone …/>` tag (the T4 taxonomy has no milestone kind; the
/// marker is the typed projection of that text channel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneMarker {
    /// The milestone's label.
    pub label: String,
}

impl MilestoneMarker {
    /// The deterministic marker form.
    #[must_use]
    pub fn render(&self) -> String {
        format!("<milestone label=\"{}\"/>", escape(&self.label))
    }

    /// Parses a marker line back (round-trip contract).
    pub fn parse(marker: &str) -> Result<Self, MilestoneParseError> {
        let body = marker
            .strip_prefix("<milestone label=\"")
            .and_then(|rest| rest.strip_suffix("\"/>"))
            .ok_or(MilestoneParseError)?;
        Ok(Self {
            label: unescape(body),
        })
    }

    /// The typed event carrying this marker.
    #[must_use]
    pub fn event(&self) -> Event {
        Event::System {
            text: self.render(),
        }
    }
}

/// Why a milestone marker failed to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("malformed milestone marker")]
pub struct MilestoneParseError;

/// The latest milestone marker in `events`, if any.
#[must_use]
pub fn latest_milestone(events: &[RecordedEvent]) -> Option<MilestoneMarker> {
    events.iter().rev().find_map(|recorded| {
        let Event::System { text } = &recorded.event else {
            return None;
        };
        MilestoneMarker::parse(text.trim()).ok()
    })
}

/// The teamwork successor package: handoff + focus-chain state + open
/// todos at a milestone boundary. A typed constructor — the daemon
/// spins the fresh successor session from it (wired later).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorBundle {
    /// The milestone boundary this package was cut at.
    pub milestone: MilestoneMarker,
    /// The distilled transfer.
    pub handoff: HandoffBundle,
    /// The focus-chain state render.
    pub focus_chain: String,
    /// The open todos remaining.
    pub open_todos: Vec<String>,
}

impl SuccessorBundle {
    /// The typed constructor cutting the package at a milestone.
    #[must_use]
    pub fn at_milestone(
        milestone: MilestoneMarker,
        handoff: HandoffBundle,
        focus_chain: String,
        open_todos: Vec<String>,
    ) -> Self {
        Self {
            milestone,
            handoff,
            focus_chain,
            open_todos,
        }
    }
}

fn escape(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn unescape(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}
