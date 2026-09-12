//! Typed conversation events, sequence numbers, and visibility classes.
//!
//! The taxonomy has seven kinds. Five are LLM-visible (user, assistant,
//! tool-call, tool-result, system) and two are internal bookkeeping
//! (condensation, snapshot) that must never be serialized into
//! model-facing context — the split is a first-class property of the type
//! ([`EventKind::class`]), not a convention.

pub mod content;
pub mod media;

pub use content::{ContentBlock, ImageBlock, InvalidMimeType, MimeType};
pub use media::{ContentHash, InvalidContentHash, MediaRef, MediaStore};

use serde::{Deserialize, Serialize};

/// Monotonic 1-based sequence number of an event within a session.
///
/// `Sequence(0)` means "no events yet"; numbering starts at
/// [`Sequence::FIRST`] and increases by exactly one per appended event.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct Sequence(pub u64);

impl Sequence {
    /// The first sequence number of any session.
    pub const FIRST: Self = Self(1);

    /// The sequence number immediately after `self`, or `None` at exhaustion.
    #[must_use]
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Visibility class of an event kind — a first-class property of the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventClass {
    /// Participates in model-facing context (user, assistant, tool-call,
    /// tool-result, system).
    LlmVisible,
    /// Harness bookkeeping (condensation, snapshot markers). Never
    /// serialized into model-facing context.
    Internal,
}

/// The kind taxonomy of conversation events (mirrors the [`Event`] variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// Input from the user.
    User,
    /// Output from the assistant model.
    Assistant,
    /// The model requesting a tool invocation.
    ToolCall,
    /// The outcome of a tool invocation.
    ToolResult,
    /// Runtime notice to the conversation.
    System,
    /// Internal condensation marker.
    Condensation,
    /// Internal snapshot marker.
    Snapshot,
}

impl EventKind {
    /// The visibility class of this kind.
    #[must_use]
    pub const fn class(self) -> EventClass {
        match self {
            Self::User | Self::Assistant | Self::ToolCall | Self::ToolResult | Self::System => {
                EventClass::LlmVisible
            }
            Self::Condensation | Self::Snapshot => EventClass::Internal,
        }
    }
}

/// A single conversation event.
///
/// The serialized form is internally tagged (`"type"`, snake_case names) and
/// is part of the durable on-disk format: tag names, field order, and sorted
/// map keys are stability contracts. No variant carries an OS path or a
/// timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Input from the user; may carry text and image blocks.
    User {
        /// Content blocks (text and/or images).
        blocks: Vec<ContentBlock>,
    },
    /// Output from the assistant model.
    Assistant {
        /// Content blocks (text and/or images).
        blocks: Vec<ContentBlock>,
    },
    /// The model requesting a tool invocation.
    ToolCall {
        /// Correlation id linking this call to its future result.
        call_id: String,
        /// Tool name.
        tool: String,
        /// Tool arguments as arbitrary JSON; object keys serialize sorted.
        arguments: serde_json::Value,
    },
    /// The outcome of a tool invocation; may carry image blocks such as
    /// screenshots.
    ToolResult {
        /// Correlation id matching the originating call.
        call_id: String,
        /// Content blocks (text and/or images).
        blocks: Vec<ContentBlock>,
    },
    /// Runtime notice to the conversation (environment facts, degradation
    /// notices). Visible to the model by design.
    System {
        /// Notice text.
        text: String,
    },
    /// Internal marker: prior history was condensed into `summary`;
    /// `through_seq` is the last sequence number covered by it.
    Condensation {
        /// Condensed-history summary.
        summary: String,
        /// Highest sequence number folded into this summary.
        through_seq: Sequence,
    },
    /// Internal marker: a named restore point in the event stream.
    Snapshot {
        /// Human-facing label for the snapshot.
        label: String,
    },
}

impl Event {
    /// The kind of this event.
    #[must_use]
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::User { .. } => EventKind::User,
            Self::Assistant { .. } => EventKind::Assistant,
            Self::ToolCall { .. } => EventKind::ToolCall,
            Self::ToolResult { .. } => EventKind::ToolResult,
            Self::System { .. } => EventKind::System,
            Self::Condensation { .. } => EventKind::Condensation,
            Self::Snapshot { .. } => EventKind::Snapshot,
        }
    }

    /// The visibility class of this event.
    #[must_use]
    pub const fn class(&self) -> EventClass {
        self.kind().class()
    }

    /// True when this event may be serialized into model-facing context.
    #[must_use]
    pub const fn is_llm_visible(&self) -> bool {
        matches!(self.class(), EventClass::LlmVisible)
    }
}

/// An event as stored in the session log: payload plus assigned sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedEvent {
    /// 1-based monotonic position within the session.
    pub seq: Sequence,
    /// The event payload.
    pub event: Event,
}

impl RecordedEvent {
    /// The visibility class of the wrapped event.
    #[must_use]
    pub const fn class(&self) -> EventClass {
        self.event.class()
    }

    /// True when the wrapped event may enter model-facing context.
    #[must_use]
    pub const fn is_llm_visible(&self) -> bool {
        self.event.is_llm_visible()
    }
}

/// Filter to the events that may be serialized into model-facing context.
///
/// This is the sanctioned path for building model context from a session:
/// internal events (condensation, snapshot markers) never pass through it.
pub fn llm_visible<'a, I>(events: I) -> impl Iterator<Item = &'a Event>
where
    I: IntoIterator<Item = &'a RecordedEvent>,
{
    events
        .into_iter()
        .filter(|recorded| recorded.event.is_llm_visible())
        .map(|recorded| &recorded.event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_next_returns_none_when_exhausted() {
        assert_eq!(Sequence::FIRST, Sequence(1));
        assert_eq!(Sequence(7).next(), Some(Sequence(8)));
        assert_eq!(Sequence(u64::MAX).next(), None);
    }
}
