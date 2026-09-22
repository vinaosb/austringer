//! Shared test instruments: the chaos bridge.
//!
//! `FakeBridge` is a real `Provider` implementation over
//! `harness-test-support`'s `FakeApi`: it maps `CompletionRequest`s ONTO the
//! T2 `ProviderRequest` shape (never forks it), serves scripted SSE replies
//! through the FakeApi rule queue, and applies per-request transport faults
//! (byte drops, clean closes) on top of the returned byte stream.

#![allow(dead_code)]

pub mod frames;
pub mod inferkit;
pub mod mediakit;
pub mod turnkit;
pub mod wirekit;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harness_providers::error::ProviderStreamError;
use harness_providers::event::ProviderEvent;
use harness_providers::identity::{CompletionRequest, ModelIdentity};
use harness_providers::provider::{EventSource, Provider, ProviderEventStream};
use harness_providers::wire::{MappedEvent, map_wire_event};
use harness_test_support::provider::{FakeApi, ProviderError, ProviderRequest};
use harness_test_support::sse::{SseByteStream, SseParser, SseScript};

/// Transport fault applied to one served reply.
#[derive(Clone, Debug)]
pub enum Fault {
    /// Serve the script untouched.
    None,
    /// Truncate the body after `n` bytes (torn final event: a dropped
    /// connection mid-stream, incl. mid-args of a tool-call delta).
    DropAfterBytes(u64),
    /// End cleanly after `n` complete events (no torn frame, no finish).
    CloseAfterEvents(usize),
}

/// A scripted reply plus the transport fault to inject when serving it.
#[derive(Clone, Debug)]
pub struct Plan {
    fault: Fault,
    kind: Kind,
}

/// What the fake answers with.
#[derive(Clone, Debug)]
enum Kind {
    Stream(SseScript),
    ServerError,
}

impl Plan {
    /// Serves `script` as-is.
    pub fn ok(script: SseScript) -> Self {
        Self {
            kind: Kind::Stream(script),
            fault: Fault::None,
        }
    }

    /// Serves `script` truncated after `n` bytes.
    pub fn dropped(script: SseScript, n: u64) -> Self {
        Self {
            kind: Kind::Stream(script),
            fault: Fault::DropAfterBytes(n),
        }
    }

    /// Serves `script` cut after `n` complete events.
    pub fn closed(script: SseScript, n: usize) -> Self {
        Self {
            kind: Kind::Stream(script),
            fault: Fault::CloseAfterEvents(n),
        }
    }

    /// Answers HTTP 500 (retry-class failure before any streaming).
    pub fn server_error() -> Self {
        Self {
            kind: Kind::ServerError,
            fault: Fault::None,
        }
    }
}

/// The chaos bridge: `Provider` over `FakeApi` with queued per-reply faults.
#[derive(Clone)]
pub struct FakeBridge {
    api: Arc<FakeApi>,
    identity: ModelIdentity,
    faults: Arc<Mutex<VecDeque<Fault>>>,
    served: Arc<Mutex<Vec<ProviderRequest>>>,
}

impl FakeBridge {
    /// Builds the bridge; every plan registers one scripted reply on the
    /// fake's catch-all rule and queues its fault, both popped in request
    /// order (the rule queue and fault queue advance in lockstep).
    pub fn new(api: Arc<FakeApi>, identity: ModelIdentity, plans: Vec<Plan>) -> Self {
        let mut rule = api.on(harness_test_support::provider::RequestMatcher::Any);
        for plan in &plans {
            rule = match &plan.kind {
                Kind::Stream(script) => rule.reply_stream(script.clone()),
                Kind::ServerError => rule.fail_server_error(),
            };
        }
        let faults = plans.iter().map(|p| p.fault.clone()).collect();
        Self {
            api,
            identity,
            faults: Arc::new(Mutex::new(faults)),
            served: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Every mapped `ProviderRequest` the fake served, in order (proves
    /// re-issues carry identical content).
    pub fn served(&self) -> Vec<ProviderRequest> {
        self.served.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn next_fault(&self) -> Fault {
        self.faults
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or(Fault::None)
    }
}

fn map_error(error: ProviderError) -> ProviderStreamError {
    match error {
        ProviderError::NoChoices => {
            ProviderStreamError::Rejected(harness_providers::error::Rejection::NoChoices)
        }
        ProviderError::OutputLimit => {
            ProviderStreamError::Rejected(harness_providers::error::Rejection::OutputLimit)
        }
        ProviderError::ContextLimit => {
            ProviderStreamError::Rejected(harness_providers::error::Rejection::ContextLimit)
        }
        ProviderError::Http {
            status,
            code,
            message,
        } => ProviderStreamError::Http {
            status,
            code,
            message,
        },
        ProviderError::UnexpectedRequest { model, prompt } => ProviderStreamError::Http {
            status: 400,
            code: "unexpected_request".to_owned(),
            message: format!("no scripted reply for model={model} prompt={prompt}"),
        },
    }
}

#[async_trait]
impl Provider for FakeBridge {
    fn identity(&self) -> &ModelIdentity {
        &self.identity
    }

    async fn stream_completion(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderEventStream, ProviderStreamError> {
        // T2 ledger constraint: the provider request shape is MAPPED ONTO,
        // not forked.
        let mut mapped = ProviderRequest::new(&request.identity.model, &request.prompt);
        for (name, value) in &request.headers {
            mapped = mapped.with_header(name, value);
        }
        if let Ok(mut served) = self.served.lock() {
            served.push(mapped.clone());
        }
        let stream = self.api.stream(&mapped).map_err(map_error)?;
        let stream = match self.next_fault() {
            Fault::None => stream,
            Fault::DropAfterBytes(n) => stream.drop_after_bytes(n),
            Fault::CloseAfterEvents(n) => stream.close_after_events(n),
        };
        Ok(ProviderEventStream::new(Box::new(SseSource {
            stream,
            parser: SseParser::new(),
            configured: self.identity.clone(),
            pending: VecDeque::new(),
        })))
    }
}

/// Pull-based typed event source over a byte stream: parse, map, buffer.
struct SseSource {
    stream: SseByteStream,
    parser: SseParser,
    configured: ModelIdentity,
    pending: VecDeque<Result<ProviderEvent, ProviderStreamError>>,
}

#[async_trait]
impl EventSource for SseSource {
    async fn next(&mut self) -> Option<Result<ProviderEvent, ProviderStreamError>> {
        loop {
            if let Some(front) = self.pending.pop_front() {
                return Some(front);
            }
            let chunk = self.stream.next_chunk().await?;
            let before = self.parser.events().len();
            self.parser.feed(&chunk);
            for parsed in &self.parser.events()[before..] {
                for mapped in map_wire_event(&parsed.event, &parsed.data, &self.configured) {
                    match mapped {
                        MappedEvent::Event(ev) => self.pending.push_back(Ok(ev)),
                        MappedEvent::Error(err) => self.pending.push_back(Err(err)),
                        MappedEvent::Skipped => {}
                    }
                }
            }
        }
    }
}

/// Script of `n` text deltas followed by usage + finish (end turn).
pub fn n_event_script(n: u32) -> SseScript {
    use harness_test_support::sse::SseScriptBuilder;
    let mut builder = SseScriptBuilder::new().event(harness_test_support::sse::ev_message_start());
    for i in 0..n {
        builder = builder.event(harness_test_support::sse::ev_text_delta(&format!(
            "tok{i} "
        )));
    }
    builder
        .event(harness_test_support::sse::ev_usage(120, n as u64))
        .event(harness_test_support::sse::ev_message_stop())
        .event(harness_test_support::sse::ev_done())
        .build()
}

/// Script of one tool call whose JSON args stream in fragments, then a
/// tool-call finish. Returns (script, call_id, name, arg_fragments).
pub fn fragmented_tool_script(
    call_id: &str,
    name: &str,
) -> (SseScript, String, String, Vec<&'static str>) {
    use harness_test_support::sse::{SseEvent, SseScriptBuilder};
    let fragments: Vec<&'static str> = vec!["{\"cmd\"", ":\"grep -r", " TODO", " src/\"}"];
    let mut builder = SseScriptBuilder::new()
        .event(harness_test_support::sse::ev_message_start())
        .event(SseEvent::new(
            "tool_call_start",
            serde_json::json!({ "call_id": call_id, "name": name }),
        ));
    for fragment in &fragments {
        builder = builder.event(harness_test_support::sse::ev_tool_call_delta(
            call_id, fragment,
        ));
    }
    let script = builder
        .event(harness_test_support::sse::ev_usage(50, 20))
        .event(SseEvent::new(
            "finish",
            serde_json::json!({ "reason": "tool_call" }),
        ))
        .event(harness_test_support::sse::ev_done())
        .build();
    (script, call_id.to_owned(), name.to_owned(), fragments)
}
