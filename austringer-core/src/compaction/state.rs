//! The persisted compaction state (plan todo 13, Cline pattern):
//! what was compacted, under which prefix hash, and the resume-time
//! validation that forces a re-compact on mismatch.
//!
//! [`sealed_prefix_hash`] pins the blake3 identity of the canonical
//! serialization of `events[seq <= through]`. Compaction saves the
//! state alongside the segments; resume re-derives the hash and a
//! mismatch is a typed [`StaleState`] — the state is never trusted
//! over the stream.

use std::fs;

use serde::{Deserialize, Serialize};

use super::segments::{SegmentError, SegmentStore};
use crate::event::{ContentHash, RecordedEvent, Sequence};

/// The persisted compaction state: what was compacted, under which
/// prefix hash, with which token accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedCompaction {
    /// The doctrine summary that replaced the sealed prefix.
    pub summary: String,
    /// The last sequence number of the sealed span (the region the
    /// summary replaced; the retained window above it is not part of
    /// the pinned prefix).
    pub through_seq: Sequence,
    /// Blake3 of the sealed prefix's canonical serialization.
    pub prefix_hash: ContentHash,
    /// Tokens before compaction.
    pub before_tokens: u64,
    /// Tokens after compaction.
    pub after_tokens: u64,
}

/// Persisted state does not match the event stream — resume must
/// re-compact, not trust the state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "stale compaction state: prefix hash mismatch through seq {} \
     (expected {}, got {}) — re-compact",
    .through_seq.0,
    .expected.to_ref(),
    .actual.to_ref()
)]
pub struct StaleState {
    /// The hash the state pinned.
    pub expected: ContentHash,
    /// The hash the stream actually produces.
    pub actual: ContentHash,
    /// The sealed watermark the state claims.
    pub through_seq: Sequence,
}

/// Blake3 of the canonical serialization of `events[seq <= through]`.
#[must_use]
pub fn sealed_prefix_hash(events: &[RecordedEvent], through: Sequence) -> ContentHash {
    let mut canonical = String::new();
    for recorded in events {
        if recorded.seq.0 <= through.0 {
            // Infallible for the event type (codebase precedent).
            canonical.push_str(&serde_json::to_string(&recorded.event).unwrap_or_default());
        }
    }
    ContentHash::from_bytes(canonical.as_bytes())
}

impl SegmentStore {
    /// The persisted state, when one exists.
    pub fn load_persisted_state(&self) -> Result<Option<PersistedCompaction>, SegmentError> {
        let path = self.compaction_dir().join("state.json");
        if !path.exists() {
            return Ok(None);
        }
        let body = fs::read_to_string(&path).map_err(|error| Self::io(&path, error))?;
        serde_json::from_str(&body).map_err(|error| SegmentError::Torn {
            file: Self::file_name(&path),
            detail: error.to_string(),
        })
    }

    /// Validates persisted state against the live stream: the sealed
    /// prefix must hash to the pinned value or the state is stale.
    pub fn validate_prefix(
        &self,
        state: &PersistedCompaction,
        events: &[RecordedEvent],
    ) -> Result<(), StaleState> {
        let actual = sealed_prefix_hash(events, state.through_seq);
        if actual == state.prefix_hash {
            Ok(())
        } else {
            Err(StaleState {
                expected: state.prefix_hash,
                actual,
                through_seq: state.through_seq,
            })
        }
    }
}
