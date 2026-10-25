//! Request matching for the rule-based fake provider.

use std::collections::BTreeMap;

/// Minimal provider-request shape used by test scripts.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderRequest {
    /// Model name the request targets.
    pub model: String,
    /// Prompt text (the last user turn in real harnesses).
    pub prompt: String,
    /// Extra request headers (e.g. auth material) recorded on cassettes.
    pub headers: BTreeMap<String, String>,
}

impl ProviderRequest {
    /// Creates a request with no headers.
    pub fn new(model: &str, prompt: &str) -> Self {
        Self {
            model: model.to_owned(),
            prompt: prompt.to_owned(),
            headers: BTreeMap::new(),
        }
    }

    /// Attaches a header (builder style).
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_owned(), value.to_owned());
        self
    }
}

/// Matcher selecting which fake-provider rule answers a request.
#[derive(Clone, Debug)]
pub enum RequestMatcher {
    /// Matches every request.
    Any,
    /// Matches when the model name equals the given value.
    Model(String),
    /// Matches when the prompt contains the given substring.
    PromptContains(String),
    /// Matches when every inner matcher matches.
    All(Vec<RequestMatcher>),
}

impl From<&str> for RequestMatcher {
    fn from(needle: &str) -> Self {
        Self::PromptContains(needle.to_owned())
    }
}

impl RequestMatcher {
    /// Creates a [`RequestMatcher::Model`].
    pub fn model(name: &str) -> Self {
        Self::Model(name.to_owned())
    }

    /// Creates a [`RequestMatcher::PromptContains`].
    pub fn prompt_contains(needle: &str) -> Self {
        Self::PromptContains(needle.to_owned())
    }

    /// Creates a [`RequestMatcher::All`] over the given matchers.
    pub fn all(inner: impl IntoIterator<Item = RequestMatcher>) -> Self {
        Self::All(inner.into_iter().collect())
    }

    /// Evaluates the matcher against a request.
    pub fn matches(&self, request: &ProviderRequest) -> bool {
        match self {
            Self::Any => true,
            Self::Model(name) => &request.model == name,
            Self::PromptContains(needle) => request.prompt.contains(needle.as_str()),
            Self::All(inner) => inner.iter().all(|m| m.matches(request)),
        }
    }
}
