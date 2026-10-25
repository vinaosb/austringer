//! The cache-violation tracker (plan todo 11, jcode SS32).
//!
//! Per turn, the expected stable prefix (blake3 hash + byte length from
//! the rendered prompt) is appended to an append-only chain and compared
//! against what the provider reported as its cache hit: a shortfall is
//! an observable [`ViolationEvent`] naming the diverged segment and byte
//! offset — cache misses become events, never silent cost spikes.
//! Providers' reports arrive through the [`CacheReportFeed`] trait; the
//! adapters implement it later, [`InMemoryFeed`] serves now.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::builder::RenderedPrompt;
use super::layout::SegmentName;
use crate::event::{ContentHash, Sequence};

/// What the provider reported for one inference call: how many prompt
/// bytes it served from cache. Adapters convert the provider's token
/// accounting to bytes when they implement the feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheReport {
    /// The turn (sequence number) the report belongs to.
    pub turn: Sequence,
    /// Bytes the provider reported as cache-hit prefix.
    pub cache_read_bytes: u64,
}

/// Source of [`CacheReport`]s. Provider adapters implement this once
/// they can observe usage accounting; the tracker consumes reports
/// through it.
pub trait CacheReportFeed {
    /// The next pending report, in turn order, if any.
    fn next_report(&mut self) -> Option<CacheReport>;
}

/// In-memory feed (tests and pre-adapter wiring).
#[derive(Debug, Clone, Default)]
pub struct InMemoryFeed {
    reports: VecDeque<CacheReport>,
}

impl InMemoryFeed {
    /// A feed that yields `reports` in the given order.
    #[must_use]
    pub fn new(reports: Vec<CacheReport>) -> Self {
        Self {
            reports: VecDeque::from(reports),
        }
    }
}

impl CacheReportFeed for InMemoryFeed {
    fn next_report(&mut self) -> Option<CacheReport> {
        self.reports.pop_front()
    }
}

/// Byte length of the longest common prefix of two strings.
#[must_use]
pub fn common_prefix_len(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// Why a violation fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationKind {
    /// This turn's prompt failed to extend the previous stable prefix
    /// (the layout law broke on our side).
    ChainBreak {
        /// The previous stable-prefix length the chain required.
        expected_common: u64,
        /// The common prefix actually measured.
        actual_common: u64,
    },
    /// The provider reused fewer bytes than the turn's common prefix
    /// with the previous prompt (the cacheable region it should have
    /// hit).
    CacheMiss {
        /// The common-prefix bytes the layout guaranteed as cacheable.
        expected_bytes: u64,
        /// The bytes the provider actually served from cache.
        reported_bytes: u64,
    },
}

/// An observable cache violation: which turn, which kind, and the first
/// diverged segment plus its byte offset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViolationEvent {
    /// The offending turn.
    pub turn: Sequence,
    /// The violation shape with its measurements.
    pub kind: ViolationKind,
    /// The segment containing the first diverged byte, when locatable.
    pub diverged_segment: Option<SegmentName>,
    /// The byte offset where divergence begins.
    pub byte_offset: u64,
}

/// One link of the append-only chain: the stable prefix a turn promised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainEntry {
    /// The turn this entry was observed on.
    pub turn: Sequence,
    /// Blake3 hash of the turn's expected stable prefix.
    pub expected_prefix_hash: ContentHash,
    /// Byte length of the expected stable prefix (L2).
    pub expected_prefix_bytes: u64,
    /// The byte length the layout guarantees to preserve into the next
    /// turn (L2 minus the rewritten history fence) — the next turn's
    /// chain-break floor.
    pub cacheable_prefix_bytes: u64,
    /// The provider's reported cache-read bytes, when a report existed.
    pub cache_read_bytes: Option<u64>,
}

/// Everything one turn's observation needs, grouped.
pub struct TurnObservation<'a> {
    /// The turn being observed.
    pub turn: Sequence,
    /// The prompt rendered for this turn.
    pub rendered: &'a RenderedPrompt,
    /// Common prefix with the previous turn's full prompt (`None` on
    /// the first turn, which establishes the chain).
    pub common_prefix_with_previous: Option<u64>,
    /// The provider's cache report for this turn, if one arrived.
    pub report: Option<CacheReport>,
}

/// The append-only stable-prefix-hash chain with violation detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheViolationTracker {
    chain: Vec<ChainEntry>,
    violations: Vec<ViolationEvent>,
}

impl CacheViolationTracker {
    /// An empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            chain: Vec::new(),
            violations: Vec::new(),
        }
    }

    /// The chain links in observation order (append-only).
    #[must_use]
    pub fn chain(&self) -> &[ChainEntry] {
        &self.chain
    }

    /// All recorded violations in observation order (append-only).
    #[must_use]
    pub fn violations(&self) -> &[ViolationEvent] {
        &self.violations
    }

    /// Records one turn: appends the chain link and checks (1) the
    /// layout law — this turn's prompt must extend the previous turn's
    /// cacheable prefix (stable content through its history end; the
    /// history fence itself is legitimately rewritten) — and (2) the
    /// provider's report — a cache read shorter than the measured common
    /// prefix is a miss. The first observation only establishes the
    /// chain: a cold cache is the baseline write, not a violation.
    /// Returns the violations fired by THIS turn; past entries are never
    /// rewritten.
    pub fn observe(&mut self, observation: TurnObservation<'_>) -> Vec<ViolationEvent> {
        let expected_bytes = observation.rendered.stable_prefix_bytes() as u64;
        let cacheable_bytes = observation.rendered.cacheable_prefix_bytes() as u64;
        let mut fired = Vec::new();
        if let (Some(previous), Some(common)) =
            (self.chain.last(), observation.common_prefix_with_previous)
            && common < previous.cacheable_prefix_bytes
        {
            fired.push(ViolationEvent {
                turn: observation.turn,
                kind: ViolationKind::ChainBreak {
                    expected_common: previous.cacheable_prefix_bytes,
                    actual_common: common,
                },
                diverged_segment: self.segment_at(observation.rendered, common),
                byte_offset: common,
            });
        }
        if let (Some(_), Some(common), Some(report)) = (
            self.chain.last(),
            observation.common_prefix_with_previous,
            observation.report,
        ) && report.cache_read_bytes < common
        {
            fired.push(ViolationEvent {
                turn: observation.turn,
                kind: ViolationKind::CacheMiss {
                    expected_bytes: common,
                    reported_bytes: report.cache_read_bytes,
                },
                diverged_segment: self.segment_at(observation.rendered, report.cache_read_bytes),
                byte_offset: report.cache_read_bytes,
            });
        }
        self.chain.push(ChainEntry {
            turn: observation.turn,
            expected_prefix_hash: observation.rendered.stable_prefix_hash,
            expected_prefix_bytes: expected_bytes,
            cacheable_prefix_bytes: cacheable_bytes,
            cache_read_bytes: observation.report.map(|report| report.cache_read_bytes),
        });
        self.violations.extend(fired.iter().cloned());
        fired
    }

    fn segment_at(&self, rendered: &RenderedPrompt, offset: u64) -> Option<SegmentName> {
        rendered
            .map
            .span_at(usize::try_from(offset).ok()?)
            .map(|span| span.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_prefix_counts_bytes_not_chars() {
        assert_eq!(common_prefix_len("abcdef", "abcXYZ"), 3);
        assert_eq!(common_prefix_len("", "abc"), 0);
        assert_eq!(common_prefix_len("same", "same"), 4);
        // Multi-byte characters count per byte, deterministically.
        assert_eq!(common_prefix_len("日本語", "日本国"), 6);
    }
}
