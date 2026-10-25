//! Turn state, the tool-side-effect dedup registry, and the resumable
//! snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::ProviderStreamError;
use crate::event::{CompletedToolCall, CompletedTurn, FinishReason, ProviderEvent, Usage};
use crate::identity::CompletionRequest;

/// Registry of tool-call side effects already executed. On resume,
/// re-delivered events for these calls are filtered: exactly-once execution
/// across interruptions (the transport may replay; the world must not).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DedupRegistry {
    executed: BTreeSet<String>,
}

impl DedupRegistry {
    /// Registers a call id as executed (idempotent).
    pub fn mark_executed(&mut self, call_id: &str) {
        self.executed.insert(call_id.to_owned());
    }

    /// Whether the call id's side effect already ran.
    #[must_use]
    pub fn contains(&self, call_id: &str) -> bool {
        self.executed.contains(call_id)
    }

    /// The executed call ids, in sorted order.
    pub fn executed(&self) -> impl Iterator<Item = &str> {
        self.executed.iter().map(String::as_str)
    }

    /// Number of registered side effects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.executed.len()
    }

    /// Whether nothing has been registered yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.executed.is_empty()
    }

    /// Whether this wire event belongs to an already-executed call and must
    /// be filtered before emission.
    #[must_use]
    pub fn filters(&self, event: &ProviderEvent) -> bool {
        match event {
            ProviderEvent::ToolCallStart { call_id, .. }
            | ProviderEvent::ToolCallDelta { call_id, .. } => self.contains(call_id),
            ProviderEvent::TextDelta { .. }
            | ProviderEvent::Usage { .. }
            | ProviderEvent::Finish { .. } => false,
        }
    }
}

/// One assembled-but-unfinished tool call inside a live turn.
#[derive(Debug, Clone, Default)]
pub(crate) struct AssembledCall {
    pub(crate) name: Option<String>,
    pub(crate) arguments: String,
}

/// The accumulated state of one turn: the resume offset (accepted count),
/// assembled output, and bookkeeping. Advanced by exactly one `apply` per
/// mapped wire event, ever — replays suppress instead of re-applying.
#[derive(Debug, Clone, Default)]
pub(crate) struct TurnState {
    pub(crate) accepted: u64,
    pub(crate) text: String,
    pub(crate) calls: BTreeMap<String, AssembledCall>,
    pub(crate) order: Vec<String>,
    pub(crate) usage: Option<Usage>,
    pub(crate) finish: Option<FinishReason>,
}

impl TurnState {
    /// Applies a delivered event (mutates output, advances the offset).
    pub(crate) fn accept(&mut self, event: ProviderEvent) {
        self.accepted = self.accepted.saturating_add(1);
        match event {
            ProviderEvent::TextDelta { text, .. } => self.text.push_str(&text),
            ProviderEvent::ToolCallStart { call_id, name, .. } => {
                if !self.calls.contains_key(&call_id) {
                    self.order.push(call_id.clone());
                }
                self.calls.entry(call_id).or_default().name = Some(name);
            }
            ProviderEvent::ToolCallDelta {
                call_id,
                arguments_fragment,
                ..
            } => {
                self.calls
                    .entry(call_id)
                    .or_default()
                    .arguments
                    .push_str(&arguments_fragment);
            }
            ProviderEvent::Usage { usage, .. } => self.usage = Some(usage),
            ProviderEvent::Finish { reason, .. } => self.finish = Some(reason),
        }
    }

    /// Advances the offset for an event consumed from the wire but neither
    /// delivered nor applied (dedup-filtered tool events of executed calls).
    pub(crate) fn advance_offset(&mut self) {
        self.accepted = self.accepted.saturating_add(1);
    }

    /// Whether the Finish event has been applied.
    #[must_use]
    pub(crate) fn is_finished(&self) -> bool {
        self.finish.is_some()
    }

    /// Assembles the complete-or-error envelope; tool calls must be named
    /// and their concatenated arguments must parse as JSON.
    pub(crate) fn envelope(
        &self,
        model: crate::identity::ModelIdentity,
    ) -> Result<CompletedTurn, ProviderStreamError> {
        let finish_reason = self.finish.ok_or(ProviderStreamError::Disconnected {
            delivered_events: self.accepted,
        })?;
        // Calls that streamed a start keep their request order; orphaned
        // calls (deltas, never a start) follow in sorted-id order so their
        // missing name surfaces deterministically.
        let mut call_ids: Vec<&String> = self.order.iter().collect();
        for call_id in self.calls.keys() {
            if !self.order.contains(call_id) {
                call_ids.push(call_id);
            }
        }
        let mut tool_calls = Vec::new();
        for call_id in call_ids {
            let assembled = self.calls.get(call_id).cloned().unwrap_or_default();
            let name = assembled
                .name
                .ok_or_else(|| ProviderStreamError::UnnamedToolCall {
                    call_id: call_id.clone(),
                })?;
            let arguments = serde_json::from_str(&assembled.arguments).map_err(|_| {
                ProviderStreamError::InvalidArguments {
                    call_id: call_id.clone(),
                }
            })?;
            tool_calls.push(CompletedToolCall {
                call_id: call_id.clone(),
                name,
                arguments,
            });
        }
        Ok(CompletedTurn {
            model,
            text: self.text.clone(),
            tool_calls,
            usage: self.usage,
            finish_reason,
        })
    }
}

/// One call as captured by a [`TurnSnapshot`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotCall {
    /// Stable correlation id.
    pub call_id: String,
    /// Tool name when a start event was applied.
    pub name: Option<String>,
    /// Raw concatenated argument fragments.
    pub arguments: String,
}

/// A serializable image of a live turn: stable request identity, accumulated
/// partial output with event offset, and the dedup registry. A new consumer
/// process can resume the same turn from it (`ResumableTurn::from_snapshot`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnSnapshot {
    /// The request this turn serves (re-issued verbatim on resume).
    pub request: CompletionRequest,
    /// Mapped provider events already in state (the replay offset).
    pub accepted: u64,
    /// Accumulated assistant text.
    pub text: String,
    /// Assembled tool calls, in request order.
    pub calls: Vec<SnapshotCall>,
    /// Usage reported so far.
    pub usage: Option<Usage>,
    /// Finish reason when the turn already completed.
    pub finish: Option<FinishReason>,
    /// Call ids whose side effects already executed.
    pub executed: Vec<String>,
    /// Attempts made before the snapshot (retry budget spent).
    pub attempts_made: u32,
    /// Resumes made before the snapshot.
    pub resumes_made: u32,
}
