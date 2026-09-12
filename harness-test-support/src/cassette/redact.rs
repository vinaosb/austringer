//! Redaction filter applied to recordings before they are persisted.

use serde_json::Value;

use super::record::{RecordedRequest, RecordedResponse};

const REDACTED: &str = "[REDACTED]";
const DEFAULT_SECRET_HEADERS: [&str; 4] = ["authorization", "x-api-key", "api-key", "cookie"];

/// Rule-based redaction for recorded requests and responses.
///
/// The default masks standard auth headers; [`mask_value`](RedactionFilter::mask_value)
/// additionally scrubs a substring everywhere it appears (prompt text and every
/// event payload), so a secret echoed by the model never lands in a cassette.
#[derive(Clone, Debug, Default)]
pub struct RedactionFilter {
    secret_headers: Vec<String>,
    needles: Vec<String>,
}

impl RedactionFilter {
    /// Default filter: masks `authorization`, `x-api-key`, `api-key`, `cookie`
    /// header values.
    pub fn new() -> Self {
        Self {
            secret_headers: DEFAULT_SECRET_HEADERS
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            needles: Vec::new(),
        }
    }

    /// Adds one more header name whose whole value must be masked.
    pub fn mask_header(mut self, name: &str) -> Self {
        self.secret_headers.push(name.to_owned());
        self
    }

    /// Adds a substring replaced by `[REDACTED]` in prompts and event payloads.
    pub fn mask_value(mut self, needle: &str) -> Self {
        self.needles.push(needle.to_owned());
        self
    }

    /// Redacts a recorded request in place (headers + prompt).
    pub fn apply_request(&self, request: &mut RecordedRequest) {
        for (name, value) in &mut request.headers {
            if self
                .secret_headers
                .iter()
                .any(|h| h.eq_ignore_ascii_case(name))
            {
                *value = REDACTED.to_owned();
            }
        }
        self.mask_string(&mut request.prompt);
    }

    /// Redacts a recorded response in place (every event payload).
    pub fn apply_response(&self, response: &mut RecordedResponse) {
        for event in &mut response.events {
            self.mask_json(&mut event.data);
        }
    }

    fn mask_string(&self, text: &mut String) {
        for needle in &self.needles {
            if text.contains(needle.as_str()) {
                *text = text.replace(needle.as_str(), REDACTED);
            }
        }
    }

    fn mask_json(&self, value: &mut Value) {
        match value {
            Value::String(text) => self.mask_string(text),
            Value::Object(map) => {
                for (_, child) in map.iter_mut() {
                    self.mask_json(child);
                }
            }
            Value::Array(items) => {
                for child in items.iter_mut() {
                    self.mask_json(child);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
}
