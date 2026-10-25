//! Recording fake-provider traffic into redacted cassettes.

use super::error::CassetteError;
use super::record::{
    Cassette, RecordedFailure, RecordedInteraction, RecordedRequest, RecordedResponse,
};
use super::redact::RedactionFilter;
use crate::provider::{ProviderRequest, ScriptedReply};
use crate::sse::SseScript;

/// Converts a scripted reply into its recorded (content-only) form.
pub(crate) fn reply_to_recorded(reply: &ScriptedReply) -> RecordedResponse {
    match reply {
        ScriptedReply::Stream(script) => RecordedResponse {
            events: script.events(),
            failure: None,
        },
        ScriptedReply::NoChoices => RecordedResponse {
            events: Vec::new(),
            failure: Some(RecordedFailure::NoChoices),
        },
        ScriptedReply::OutputLimit => RecordedResponse {
            events: Vec::new(),
            failure: Some(RecordedFailure::OutputLimit),
        },
        ScriptedReply::ContextLimit => RecordedResponse {
            events: Vec::new(),
            failure: Some(RecordedFailure::ContextLimit),
        },
        ScriptedReply::ServerError => RecordedResponse {
            events: Vec::new(),
            failure: Some(RecordedFailure::ServerError),
        },
    }
}

/// Converts a recorded response back into a live scripted reply.
pub(crate) fn recorded_to_reply(recorded: &RecordedResponse) -> ScriptedReply {
    match recorded.failure {
        Some(RecordedFailure::NoChoices) => ScriptedReply::NoChoices,
        Some(RecordedFailure::OutputLimit) => ScriptedReply::OutputLimit,
        Some(RecordedFailure::ContextLimit) => ScriptedReply::ContextLimit,
        Some(RecordedFailure::ServerError) => ScriptedReply::ServerError,
        None => ScriptedReply::Stream(SseScript::from_events(recorded.events.clone())),
    }
}

/// Accumulates `(request, reply)` pairs into a redacted cassette.
///
/// Redaction happens at record time, so an in-flight or saved cassette never
/// holds unmasked secrets.
#[derive(Debug)]
pub struct CassetteRecorder {
    cassette: Cassette,
    filter: RedactionFilter,
}

impl CassetteRecorder {
    /// Creates a recorder with the default redaction filter.
    pub fn new(name: &str) -> Self {
        Self::with_filter(name, RedactionFilter::new())
    }

    /// Creates a recorder with an explicit redaction filter.
    pub fn with_filter(name: &str, filter: RedactionFilter) -> Self {
        Self {
            cassette: Cassette {
                name: name.to_owned(),
                interactions: Vec::new(),
            },
            filter,
        }
    }

    /// Records one interaction: the request and reply are copied, redacted,
    /// and appended in order.
    pub fn record(&mut self, request: &ProviderRequest, reply: &ScriptedReply) {
        let mut recorded_request = RecordedRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            headers: request.headers.clone(),
        };
        let mut recorded_response = reply_to_recorded(reply);
        self.filter.apply_request(&mut recorded_request);
        self.filter.apply_response(&mut recorded_response);
        self.cassette.interactions.push(RecordedInteraction {
            request: recorded_request,
            response: recorded_response,
        });
    }

    /// Finishes recording and returns the redacted cassette.
    pub fn finish(self) -> Cassette {
        self.cassette
    }

    /// Convenience: finishes and saves in one call.
    pub fn save(self, path: &std::path::Path) -> Result<Cassette, CassetteError> {
        let cassette = self.cassette;
        cassette.save(path)?;
        Ok(cassette)
    }
}
