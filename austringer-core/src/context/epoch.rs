//! The context-epoch vocabulary (plan todo 11).
//!
//! A [`ContextEpoch`] is the named, immutable provider-cache baseline
//! (Kilo CONTEXT.md doctrine, research SS11): the frozen segment hashes
//! plus the pinned effort at its start. It survives turn boundaries —
//! history grows append-only — and ends ONLY at a completed compaction or
//! session movement, both explicit typed transitions recorded by the
//! [`EpochLedger`]. Durable mid-conversation updates append IN-HISTORY
//! as events; mutating a frozen segment mid-epoch surfaces as typed
//! [`EpochDrift`] instead.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::builder::RenderedPrompt;
use super::effort::{EFFORT_REMEDIES, EffortSettings, EffortViolation};
use super::layout::SegmentName;
use crate::event::{ContentHash, Sequence};

/// Identity of a context epoch; sequential from 1 within a ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EpochId(pub u64);

/// The immutable baseline of an epoch: per-segment hashes of the frozen
/// segments plus the full stable-prefix hash at epoch start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableBaseline {
    /// Frozen segment hashes in canonical order.
    pub segment_hashes: Vec<(SegmentName, ContentHash)>,
    /// Hash of the whole `[0,L2)` stable prefix at epoch start.
    pub prefix_hash: ContentHash,
}

impl StableBaseline {
    /// Pins the baseline of a rendered prompt.
    #[must_use]
    pub fn from_rendered(rendered: &RenderedPrompt) -> Self {
        Self {
            segment_hashes: rendered.frozen_hashes(),
            prefix_hash: rendered.stable_prefix_hash,
        }
    }
}

/// One context epoch: the cache baseline plus the effort pinned to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextEpoch {
    /// The epoch's identity.
    pub id: EpochId,
    /// The frozen baseline pinned at epoch start.
    pub baseline: StableBaseline,
    /// The effort settings rendered into the system segment for this
    /// epoch's whole lifetime.
    pub effort: EffortSettings,
    /// The first sequence number covered by this epoch.
    pub started_at_seq: Sequence,
}

impl ContextEpoch {
    /// The baseline hash of one frozen segment, when pinned.
    #[must_use]
    pub fn baseline_hash(&self, name: SegmentName) -> Option<ContentHash> {
        self.baseline
            .segment_hashes
            .iter()
            .find(|(segment, _)| *segment == name)
            .map(|(_, hash)| *hash)
    }

    /// Validates a rendered turn against this epoch: effort must match
    /// the pinned settings (checked first — on cache-coupled providers a
    /// change invalidates every breakpoint, so the caller gets the
    /// actionable effort rejection rather than generic drift), and every
    /// frozen segment must still hash to its baseline.
    pub fn check(
        &self,
        rendered: &RenderedPrompt,
        requested: &EffortSettings,
    ) -> Result<(), EpochError> {
        if *requested != self.effort {
            return Err(EpochError::Effort(EffortViolation {
                current: self.effort.clone(),
                requested: requested.clone(),
                epoch: self.id,
                remedies: EFFORT_REMEDIES,
            }));
        }
        for (segment, expected) in &self.baseline.segment_hashes {
            if let Some(actual) = rendered.segment_hash(*segment)
                && actual != *expected
            {
                return Err(EpochError::Drift(EpochDrift {
                    segment: *segment,
                    expected: *expected,
                    actual,
                }));
            }
        }
        Ok(())
    }
}

/// The only two ways an epoch ends (both explicit events).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EpochTransition {
    /// A compaction completed through this sequence number.
    CompactionCompleted {
        /// The last sequence number folded into the compaction.
        through_seq: Sequence,
    },
    /// The session moved (fork/resume/handoff) at this sequence number.
    SessionMovement {
        /// The sequence number at which the session moved.
        moved_at_seq: Sequence,
    },
}

impl EpochTransition {
    /// The first sequence number of the successor epoch (clamped at
    /// sequence exhaustion).
    #[must_use]
    pub fn resume_seq(&self) -> Sequence {
        let boundary = match self {
            Self::CompactionCompleted { through_seq }
            | Self::SessionMovement {
                moved_at_seq: through_seq,
            } => *through_seq,
        };
        boundary.next().unwrap_or(Sequence(u64::MAX))
    }
}

/// A frozen segment mutated mid-epoch. The typed payload names the
/// segment and both hashes; the Display text carries the doctrine
/// (append in-history, or end the epoch through a completed compaction).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct EpochDrift {
    /// The frozen segment whose hash moved.
    pub segment: SegmentName,
    /// The pinned baseline hash.
    pub expected: ContentHash,
    /// The hash actually rendered this turn.
    pub actual: ContentHash,
}

impl fmt::Display for EpochDrift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "epoch drift in {:?}: expected {}, got {} — append durable \
             updates in-history or end the epoch via a completed compaction",
            self.segment,
            self.expected.to_ref(),
            self.actual.to_ref()
        )
    }
}

/// Why an epoch check failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EpochError {
    /// A frozen segment mutated mid-epoch.
    #[error("{0}")]
    Drift(#[from] EpochDrift),
    /// Effort changed mid-epoch (would invalidate every breakpoint).
    #[error("{0}")]
    Effort(#[from] EffortViolation),
}

/// One recorded epoch transition event (append-only ledger entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochTransitionRecord {
    /// The epoch that ended.
    pub from: EpochId,
    /// The transition that ended it.
    pub transition: EpochTransition,
    /// The epoch that began.
    pub to: EpochId,
}

/// The append-only record of a session's epochs and their transitions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochLedger {
    epochs: Vec<ContextEpoch>,
    transitions: Vec<EpochTransitionRecord>,
}

impl EpochLedger {
    /// An empty ledger (no epoch yet).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            epochs: Vec::new(),
            transitions: Vec::new(),
        }
    }

    /// All epochs in order.
    #[must_use]
    pub fn epochs(&self) -> &[ContextEpoch] {
        &self.epochs
    }

    /// The current epoch, if any.
    #[must_use]
    pub fn current(&self) -> Option<&ContextEpoch> {
        self.epochs.last()
    }

    /// All recorded transitions in order.
    #[must_use]
    pub fn transitions(&self) -> &[EpochTransitionRecord] {
        &self.transitions
    }

    /// Pins the first epoch (or the next after a transition driven
    /// externally) from a rendered prompt.
    pub fn begin(
        &mut self,
        rendered: &RenderedPrompt,
        effort: EffortSettings,
        started_at_seq: Sequence,
    ) -> ContextEpoch {
        let epoch = ContextEpoch {
            id: EpochId(self.next_id()),
            baseline: StableBaseline::from_rendered(rendered),
            effort,
            started_at_seq,
        };
        self.epochs.push(epoch.clone());
        epoch
    }

    /// Ends the current epoch with an explicit transition and pins the
    /// successor from the post-transition render. The rendered state at
    /// the boundary becomes the new baseline, so frozen content and
    /// effort may legitimately change here and only here.
    pub fn transition(
        &mut self,
        transition: EpochTransition,
        rendered: &RenderedPrompt,
        effort: EffortSettings,
    ) -> ContextEpoch {
        let to = EpochId(self.next_id());
        let from = self.current().map_or(to, |epoch| epoch.id);
        let epoch = ContextEpoch {
            id: to,
            baseline: StableBaseline::from_rendered(rendered),
            effort,
            started_at_seq: transition.resume_seq(),
        };
        self.transitions.push(EpochTransitionRecord {
            from,
            transition,
            to,
        });
        self.epochs.push(epoch.clone());
        epoch
    }

    fn next_id(&self) -> u64 {
        self.epochs.last().map_or(1, |epoch| epoch.id.0 + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_seq_is_one_past_the_boundary() {
        assert_eq!(
            EpochTransition::CompactionCompleted {
                through_seq: Sequence(8)
            }
            .resume_seq(),
            Sequence(9)
        );
        assert_eq!(
            EpochTransition::SessionMovement {
                moved_at_seq: Sequence(u64::MAX)
            }
            .resume_seq(),
            Sequence(u64::MAX)
        );
    }
}
