//! The fixed prompt layout (plan todo 11).
//!
//! Every prompt the builder emits tiles exactly seven segments in a fixed
//! order — `[0,L0)` system + base tool schema, `[L0,L1)` rules + skills
//! slots, `[L1,L2)` repomap + history, and the volatile tail (the last
//! user message) after L2. [`SegmentMap`] is the typed segment-boundary
//! record adapters consume; provider cache-control metadata lives in
//! [`super::cache_plan`].

use serde::{Deserialize, Serialize};

/// One of the seven fixed segments of the canonical prompt layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SegmentName {
    /// The system prompt text plus the rendered effort settings.
    System,
    /// The T6 base tool schema (byte-identical to its render).
    ToolSchema,
    /// Normalized rules slot segments.
    Rules,
    /// Normalized skills slot segments.
    Skills,
    /// The repository map.
    Repomap,
    /// Append-only LLM-visible history events.
    History,
    /// The volatile tail: the last user message, the only mutable region.
    VolatileTail,
}

impl SegmentName {
    /// The canonical XML-ish tag wrapping this segment's content.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::ToolSchema => "tool-schema",
            Self::Rules => "rules",
            Self::Skills => "skills",
            Self::Repomap => "repomap",
            Self::History => "history",
            Self::VolatileTail => "user-message",
        }
    }

    /// True for the segments frozen for a context epoch's lifetime
    /// (everything the baseline hashes; history grows append-only and the
    /// tail mutates freely).
    #[must_use]
    pub const fn is_frozen(self) -> bool {
        matches!(
            self,
            Self::System | Self::ToolSchema | Self::Rules | Self::Skills | Self::Repomap
        )
    }

    /// The frozen segments in canonical order.
    pub const FROZEN: [Self; 5] = [
        Self::System,
        Self::ToolSchema,
        Self::Rules,
        Self::Skills,
        Self::Repomap,
    ];
}

/// Whether a segment is byte-stable within an epoch or volatile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stability {
    /// Byte-identical for the same logical state within an epoch.
    Stable,
    /// Allowed to mutate (only the volatile tail).
    Volatile,
}

/// One contiguous segment span in the rendered prompt, covering its full
/// tagged block (`<tag>\n...\n</tag>\n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentSpan {
    /// Which segment this is.
    pub name: SegmentName,
    /// Byte offset of the opening tag.
    pub start: usize,
    /// Block length in bytes.
    pub len: usize,
    /// Stable-vs-volatile classification.
    pub stability: Stability,
}

impl SegmentSpan {
    /// The byte offset just past the closing tag.
    #[must_use]
    pub const fn end(self) -> usize {
        self.start + self.len
    }
}

/// The typed segment-boundary record for one rendered prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentMap {
    spans: Vec<SegmentSpan>,
    l0: usize,
    l1: usize,
    l2: usize,
}

impl SegmentMap {
    /// Builds the map from the seven spans in canonical order, deriving
    /// the L0/L1/L2 boundary offsets. The builder is the only caller and
    /// always supplies every canonical span; a missing span yields the
    /// boundary 0, which the byte-identity tests would catch immediately.
    pub(crate) fn new(spans: Vec<SegmentSpan>) -> Self {
        let end_of = |name: SegmentName| {
            spans
                .iter()
                .find(|span| span.name == name)
                .map_or(0, |span| span.end())
        };
        let (l0, l1, l2) = (
            end_of(SegmentName::ToolSchema),
            end_of(SegmentName::Skills),
            end_of(SegmentName::History),
        );
        Self { spans, l0, l1, l2 }
    }

    /// All spans in canonical order.
    #[must_use]
    pub fn spans(&self) -> &[SegmentSpan] {
        &self.spans
    }

    /// The span for `name`.
    #[must_use]
    pub fn span(&self, name: SegmentName) -> Option<&SegmentSpan> {
        self.spans.iter().find(|span| span.name == name)
    }

    /// The span containing byte `offset` (`start <= offset < end`), i.e.
    /// the segment the byte at `offset` belongs to.
    #[must_use]
    pub fn span_at(&self, offset: usize) -> Option<&SegmentSpan> {
        self.spans
            .iter()
            .find(|span| offset >= span.start && offset < span.end())
    }

    /// End of `[0,L0)`: system + base tool schema.
    #[must_use]
    pub const fn l0(&self) -> usize {
        self.l0
    }

    /// End of `[L0,L1)`: rules + skills slots.
    #[must_use]
    pub const fn l1(&self) -> usize {
        self.l1
    }

    /// End of `[L1,L2)`: repomap + history — the whole stable prefix.
    #[must_use]
    pub const fn l2(&self) -> usize {
        self.l2
    }

    /// The stable-prefix length (everything above the volatile tail).
    #[must_use]
    pub const fn stable_prefix_len(&self) -> usize {
        self.l2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(name: SegmentName, start: usize, len: usize) -> SegmentSpan {
        SegmentSpan {
            name,
            start,
            len,
            stability: if name == SegmentName::VolatileTail {
                Stability::Volatile
            } else {
                Stability::Stable
            },
        }
    }

    #[test]
    fn boundaries_and_lookup_follow_the_spans() {
        let lengths = [10usize, 20, 5, 0, 8, 40, 6];
        let names = [
            SegmentName::System,
            SegmentName::ToolSchema,
            SegmentName::Rules,
            SegmentName::Skills,
            SegmentName::Repomap,
            SegmentName::History,
            SegmentName::VolatileTail,
        ];
        let mut spans = Vec::new();
        let mut cursor = 0;
        for (index, name) in names.into_iter().enumerate() {
            spans.push(span(name, cursor, lengths[index]));
            cursor += lengths[index];
        }
        let map = SegmentMap::new(spans);
        assert_eq!(map.l0(), 30);
        assert_eq!(map.l1(), 35);
        assert_eq!(map.stable_prefix_len(), map.l2());
        assert_eq!(map.l2(), 83);
        assert_eq!(
            map.span_at(34).map(|s| s.name),
            Some(SegmentName::Rules),
            "byte 34 is still inside rules (35 is the boundary)"
        );
        assert_eq!(map.span_at(35).map(|s| s.name), Some(SegmentName::Repomap));
        assert_eq!(map.span_at(100), None);
        assert!(SegmentName::FROZEN.iter().all(|name| name.is_frozen()));
        assert!(!SegmentName::History.is_frozen());
        assert!(!SegmentName::VolatileTail.is_frozen());
    }
}
