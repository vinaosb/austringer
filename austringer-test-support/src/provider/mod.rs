//! Rule-based fake provider (goose `DummyApi` pattern):
//! `api.on(matcher).reply_stream(script).up_to_n_times(n)`.
//!
//! Entirely in-memory: rules are matched against [`ProviderRequest`]s and
//! answers are served as [`SseByteStream`]s. No sockets, no network.

mod error;
mod matcher;
mod reply;

use std::collections::VecDeque;
use std::sync::Mutex;

pub use error::ProviderError;
pub use matcher::{ProviderRequest, RequestMatcher};
pub use reply::ScriptedReply;

use crate::sse::{SseByteStream, SseScript};

/// One registered rule: matcher, queued replies, and an optional use cap.
#[derive(Debug)]
struct Rule {
    matcher: RequestMatcher,
    queue: VecDeque<ScriptedReply>,
    max_uses: Option<u32>,
    used: u32,
}

/// The fake provider: an ordered list of rules consulted per request.
///
/// A rule serves a request when its matcher matches, it still has queued
/// replies, and its `up_to_n_times` cap (if set) is not exhausted. The first
/// such rule in registration order wins; with no candidate the request fails
/// with [`ProviderError::UnexpectedRequest`].
#[derive(Debug, Default)]
pub struct FakeApi {
    rules: Mutex<Vec<Rule>>,
}

impl FakeApi {
    /// Creates an empty fake provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an empty rule for `matcher` and returns its builder.
    pub fn on(&self, matcher: impl Into<RequestMatcher>) -> RuleBuilder<'_> {
        let mut rules = self.lock();
        rules.push(Rule {
            matcher: matcher.into(),
            queue: VecDeque::new(),
            max_uses: None,
            used: 0,
        });
        RuleBuilder {
            api: self,
            index: rules.len() - 1,
        }
    }

    /// Resolves the scripted reply for a request (pops it off its rule).
    fn take_reply(&self, request: &ProviderRequest) -> Result<ScriptedReply, ProviderError> {
        let mut rules = self.lock();
        for rule in rules.iter_mut() {
            let uses_left = rule.max_uses.is_none_or(|max| rule.used < max);
            if uses_left
                && rule.matcher.matches(request)
                && let Some(reply) = rule.queue.pop_front()
            {
                rule.used += 1;
                return Ok(reply);
            }
        }
        Err(ProviderError::UnexpectedRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Rule>> {
        // Poisoning is impossible in practice: no code panics while holding the
        // lock (critical sections are plain pushes/pops).
        self.rules
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl FakeApi {
    /// Serves a request as a scripted SSE byte stream, or fails immediately
    /// with the typed error of the scripted failure mode.
    pub fn stream(&self, request: &ProviderRequest) -> Result<SseByteStream, ProviderError> {
        match self.take_reply(request)? {
            ScriptedReply::Stream(script) => Ok(SseByteStream::from_script(&script)),
            ScriptedReply::NoChoices => Err(ProviderError::NoChoices),
            ScriptedReply::OutputLimit => Err(ProviderError::OutputLimit),
            ScriptedReply::ContextLimit => Err(ProviderError::ContextLimit),
            ScriptedReply::ServerError => Err(ProviderError::Http {
                status: 500,
                code: "server_error".to_owned(),
                message: "scripted server failure".to_owned(),
            }),
        }
    }

    /// Streams a request to completion and joins all text deltas into the
    /// assistant message.
    pub async fn complete(&self, request: &ProviderRequest) -> Result<String, ProviderError> {
        let (text, _) = self.stream(request)?.read_all().await;
        Ok(text)
    }
}

/// Builder returned by [`FakeApi::on`]; every `reply_*`/`fail_*` call appends
/// one queued reply to the rule.
#[derive(Debug)]
pub struct RuleBuilder<'a> {
    api: &'a FakeApi,
    index: usize,
}

impl RuleBuilder<'_> {
    /// Queues a successful scripted SSE stream.
    pub fn reply_stream(self, script: SseScript) -> Self {
        self.push(ScriptedReply::Stream(script))
    }

    /// Queues the NoChoices failure (200 OK, zero choices).
    pub fn fail_no_choices(self) -> Self {
        self.push(ScriptedReply::NoChoices)
    }

    /// Queues the OutputLimit failure (`finish_reason=length`).
    pub fn fail_output_limit(self) -> Self {
        self.push(ScriptedReply::OutputLimit)
    }

    /// Queues the ContextLimit failure (HTTP 413, `context_length_exceeded`).
    pub fn fail_context_limit(self) -> Self {
        self.push(ScriptedReply::ContextLimit)
    }

    /// Queues a generic HTTP 500 failure.
    pub fn fail_server_error(self) -> Self {
        self.push(ScriptedReply::ServerError)
    }

    /// Queues one successful reply followed by a server error — the classic
    /// "works once, then dies" sequence for retry-exhaustion tests.
    pub fn reply_then_server_error(self, script: SseScript) -> Self {
        self.push(ScriptedReply::Stream(script))
            .push(ScriptedReply::ServerError)
    }

    /// Caps how many times this rule may serve requests.
    pub fn up_to_n_times(self, n: u32) -> Self {
        self.api.lock()[self.index].max_uses = Some(n);
        self
    }

    fn push(self, reply: ScriptedReply) -> Self {
        self.api.lock()[self.index].queue.push_back(reply);
        self
    }
}
