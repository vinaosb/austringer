//! Serializable cassette data model. Maps are `BTreeMap`s so serialized keys
//! are sorted and cassettes render deterministically.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::sse::SseEvent;

/// One recorded request, already passed through the redaction filter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedRequest {
    /// Model name.
    pub model: String,
    /// Full prompt text (post-redaction).
    pub prompt: String,
    /// Request headers (post-redaction).
    pub headers: BTreeMap<String, String>,
}

/// Scripted failure kind stored on cassettes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedFailure {
    /// HTTP 200 with zero choices.
    NoChoices,
    /// `finish_reason == length`.
    OutputLimit,
    /// HTTP 413 `context_length_exceeded`.
    ContextLimit,
    /// HTTP 500.
    ServerError,
}

/// One recorded response: streamed events and/or a scripted failure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedResponse {
    /// Streamed events in order (empty for immediate failures). Timing is not
    /// recorded — cassettes capture content, not delays.
    pub events: Vec<SseEvent>,
    /// Failure kind when the reply was an error injection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<RecordedFailure>,
}

/// One request/response pair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedInteraction {
    /// The request.
    pub request: RecordedRequest,
    /// The reply.
    pub response: RecordedResponse,
}

/// A named, ordered list of interactions — the unit saved to disk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cassette {
    /// Cassette name, echoed in every strict-replay error.
    pub name: String,
    /// Interactions in strict replay order.
    pub interactions: Vec<RecordedInteraction>,
}
