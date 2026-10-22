//! M32: cassette-backed provider transport (in-memory, zero network)
//! and the anthropic-wire text-only cassette builder.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use harness_providers::transport::{
    Body, ByteSource, ByteStream, Transport, TransportFailure, TransportRequest, TransportResponse,
};
use harness_test_support::sse::{SseByteStream, SseScript};

/// One staged provider reply for [`CassetteTransport`].
pub enum Cassette {
    /// HTTP 200 carrying the scripted SSE stream.
    Sse(SseScript),
    /// A send that never resolves (hung provider, adversarial tier).
    Hang,
}

struct CassetteSource(SseByteStream);

#[async_trait::async_trait]
impl ByteSource for CassetteSource {
    async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        self.0.next_chunk().await
    }
}

/// In-memory transport serving staged cassette replies in order and
/// recording every rendered request (URL, headers, body) for assertions.
pub struct CassetteTransport {
    queue: Mutex<VecDeque<Cassette>>,
    requests: Mutex<Vec<TransportRequest>>,
}

impl CassetteTransport {
    /// Builds the transport over `staged` (served in arrival order).
    pub fn new(staged: Vec<Cassette>) -> Arc<Self> {
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
}

#[async_trait::async_trait]
impl Transport for CassetteTransport {
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
            Some(Cassette::Sse(script)) => {
                Ok(TransportResponse::ok(Body::Stream(ByteStream::new(
                    Box::new(CassetteSource(SseByteStream::from_script(&script))),
                ))))
            }
            Some(Cassette::Hang) => futures_util::future::pending().await,
            None => Err(TransportFailure::Connect("cassette exhausted".to_owned())),
        }
    }
}

/// Anthropic-wire text-only cassette: usage-carrying start, one text
/// delta, end_turn finish, stop. The distinctive answer text is what
/// e2e assertions pin on the wire.
pub fn anthropic_text_cassette(model: &str, answer: &str) -> SseScript {
    let frame = |event: &str, data: serde_json::Value| {
        harness_test_support::sse::SseEvent::new(event, data)
    };
    SseScript::from_events([
        frame(
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "message": {"model": model, "usage": {"input_tokens": 12, "output_tokens": 1}},
            }),
        ),
        frame(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta", "text": answer},
            }),
        ),
        frame(
            "message_delta",
            serde_json::json!({
                "type": "message_delta",
                "delta": {"stop_reason": "end_turn"},
                "usage": {"output_tokens": 7},
            }),
        ),
        frame("message_stop", serde_json::json!({"type": "message_stop"})),
    ])
}
