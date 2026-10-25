//! Shared kit for adapter tests: a scripted in-memory [`Transport`] over the
//! T2 chaos instruments plus the turn/stream drivers. Vendor frame builders
//! live in [`super::frames`].

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use austringer_providers::auth::NoCredentials;
use austringer_providers::error::ProviderStreamError;
use austringer_providers::event::{ProviderEvent, Usage};
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::provider::{Provider, ProviderEventStream};
use austringer_providers::retry::FixedJitter;
use austringer_providers::transport::{
    Body, ByteSource, ByteStream, Transport, TransportFailure, TransportRequest, TransportResponse,
};
use austringer_providers::turn::{RecoveryConfig, ResumableTurn};
use austringer_test_support::sse::{SseByteStream, SseScript};
use serde_json::Value;

/// Transport chaos applied to a staged SSE reply.
#[derive(Clone, Debug)]
pub enum Chaos {
    /// Serve the script untouched.
    None,
    /// Truncate the body after `n` bytes (torn final frame).
    DropAfterBytes(u64),
    /// End cleanly after `n` complete events.
    CloseAfterEvents(usize),
}

/// One staged transport reply.
#[derive(Clone)]
pub enum Staged {
    /// 200 with the scripted SSE stream (plus optional chaos).
    Sse(SseScript, Chaos),
    /// An HTTP error status with a JSON error body.
    HttpError(u16, Value),
    /// A transport-level failure (connect refused, timeout, ...).
    Transport(TransportFailure),
}

impl Staged {
    /// A clean 200 SSE stream.
    pub fn sse(script: SseScript) -> Self {
        Self::Sse(script, Chaos::None)
    }
}

/// Adapter of T2's `SseByteStream` onto the providers `ByteSource` seam.
struct ScriptedSource(SseByteStream);

#[async_trait]
impl ByteSource for ScriptedSource {
    async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        self.0.next_chunk().await
    }
}

/// In-memory transport serving staged replies in order; records every
/// rendered request for body/header assertions.
#[derive(Default)]
pub struct ScriptedTransport {
    queue: Mutex<VecDeque<Staged>>,
    requests: Mutex<Vec<TransportRequest>>,
}

impl ScriptedTransport {
    /// Builds the transport over `staged` (served in order).
    pub fn new(staged: Vec<Staged>) -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(staged.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        })
    }

    /// Every rendered request, in arrival order.
    pub fn requests(&self) -> Vec<TransportRequest> {
        self.requests
            .lock()
            .map(|reqs| reqs.clone())
            .unwrap_or_default()
    }

    /// The JSON body of the n-th rendered request.
    pub fn body_json(&self, index: usize) -> Value {
        let body = &self.requests()[index].body;
        serde_json::from_slice(body).unwrap_or(Value::Null)
    }
}

#[async_trait]
impl Transport for ScriptedTransport {
    async fn send(&self, request: TransportRequest) -> Result<TransportResponse, TransportFailure> {
        if let Ok(mut reqs) = self.requests.lock() {
            reqs.push(request);
        }
        let staged = self
            .queue
            .lock()
            .ok()
            .and_then(|mut queue| queue.pop_front());
        match staged {
            Some(Staged::Sse(script, chaos)) => {
                let stream = SseByteStream::from_script(&script);
                let stream = match chaos {
                    Chaos::None => stream,
                    Chaos::DropAfterBytes(n) => stream.drop_after_bytes(n),
                    Chaos::CloseAfterEvents(n) => stream.close_after_events(n),
                };
                Ok(TransportResponse::ok(Body::Stream(ByteStream::new(
                    Box::new(ScriptedSource(stream)),
                ))))
            }
            Some(Staged::HttpError(status, body)) => Ok(TransportResponse::with_status(
                status,
                Body::Full(serde_json::to_vec(&body).unwrap_or_default()),
            )),
            Some(Staged::Transport(failure)) => Err(failure),
            None => Err(TransportFailure::Connect("script exhausted".to_owned())),
        }
    }
}

/// Collects a provider stream to its end (events + terminal error, if any).
pub async fn collect(
    mut stream: ProviderEventStream,
) -> (Vec<ProviderEvent>, Option<ProviderStreamError>) {
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(event) => events.push(event),
            Err(err) => return (events, Some(err)),
        }
    }
    (events, None)
}

/// Drives a full turn through the recovery layer with deterministic backoff.
pub async fn drive_turn(
    provider: Arc<dyn Provider>,
    identity: ModelIdentity,
    prompt: &str,
    retries: u32,
) -> (Vec<ProviderEvent>, Result<(), ProviderStreamError>) {
    let config = RecoveryConfig {
        retry: austringer_providers::retry::RetryPolicy {
            max_attempts: retries,
            initial_backoff: std::time::Duration::from_millis(1),
            max_backoff: std::time::Duration::from_millis(4),
            multiplier: 2.0,
        },
        max_resumes: 4,
        idle_timeout: std::time::Duration::from_secs(60),
    };
    let mut turn = ResumableTurn::start(
        provider,
        CompletionRequest::new(identity, prompt),
        config,
        Box::new(FixedJitter::new([0.5])),
    );
    let mut events = Vec::new();
    while let Some(item) = turn.next_event().await {
        match item {
            Ok(event) => events.push(event),
            Err(err) => return (events, Err(err)),
        }
    }
    (events, Ok(()))
}

/// Extracts the single usage event from a delivered event sequence.
pub fn usage_of(events: &[ProviderEvent]) -> Usage {
    events
        .iter()
        .find_map(|event| match event {
            ProviderEvent::Usage { usage, .. } => Some(*usage),
            _ => None,
        })
        .unwrap_or_default()
}

/// Joins delivered text deltas.
pub fn text_of(events: &[ProviderEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            ProviderEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

pub fn no_auth() -> Arc<NoCredentials> {
    Arc::new(NoCredentials)
}
