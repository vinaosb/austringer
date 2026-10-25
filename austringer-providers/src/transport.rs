//! The hermetic transport seam: request/response types generic over the HTTP
//! client, so adapters are testable against in-memory transports and the
//! production reqwest client interchangeably (todo 10's critical design).
//!
//! Adapters never open sockets themselves: they render a
//! [`TransportRequest`] and consume a [`TransportResponse`] through whatever
//! [`Transport`] was injected. Zero network in tests is enforced by
//! construction — the test transports are pure in-memory state machines.

use std::collections::BTreeMap;

use async_trait::async_trait;

/// One outbound HTTP request: full URL, headers, and a fully-buffered body
/// (request bodies are rendered JSON — always complete before sending).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportRequest {
    /// Absolute request URL.
    pub url: String,
    /// Request headers (auth material, content type, vendor versions).
    pub headers: BTreeMap<String, String>,
    /// Serialized request body.
    pub body: Vec<u8>,
}

impl TransportRequest {
    /// Creates a request for `url` with `body` and no headers.
    #[must_use]
    pub fn new(url: &str, body: Vec<u8>) -> Self {
        Self {
            url: url.to_owned(),
            headers: BTreeMap::new(),
            body,
        }
    }

    /// Attaches a header (builder style).
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_owned(), value.to_owned());
        self
    }
}

/// Pull-based source of body chunks. Mirrors the shape of a chunked HTTP
/// response body; dropping the owning [`ByteStream`] drops the source.
#[async_trait]
pub trait ByteSource: Send {
    /// Yields the next chunk, or `None` at end of body.
    async fn next_chunk(&mut self) -> Option<Vec<u8>>;
}

/// A streaming response body.
pub struct ByteStream {
    source: Box<dyn ByteSource>,
}

impl std::fmt::Debug for ByteStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("ByteStream(..)")
    }
}

impl ByteStream {
    /// Wraps a chunk source.
    #[must_use]
    pub fn new(source: Box<dyn ByteSource>) -> Self {
        Self { source }
    }

    /// Yields the next body chunk, or `None` at end of body.
    pub async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        self.source.next_chunk().await
    }
}

/// A response body: either fully buffered or streaming.
#[derive(Debug)]
pub enum Body {
    /// Complete body bytes (error payloads, non-streaming replies).
    Full(Vec<u8>),
    /// Chunk-streamed body (SSE).
    Stream(ByteStream),
}

/// One HTTP response: status plus body.
pub struct TransportResponse {
    /// HTTP status code.
    pub status: u16,
    /// The response body.
    pub body: Body,
}

impl TransportResponse {
    /// A 200 response carrying `body`.
    #[must_use]
    pub fn ok(body: Body) -> Self {
        Self { status: 200, body }
    }

    /// A response with an explicit status (error payloads).
    #[must_use]
    pub fn with_status(status: u16, body: Body) -> Self {
        Self { status, body }
    }
}

/// Transport-level failure taxonomy: everything that can go wrong before an
/// HTTP response exists, plus framing violations on the response path.
/// All variants classify as retryable at the provider layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportFailure {
    /// Could not reach the endpoint (DNS, connect refused, proxy).
    #[error("connection failure: {0}")]
    Connect(String),
    /// A deadline elapsed before the request completed.
    #[error("timeout: {0}")]
    Timeout(String),
    /// TLS handshake or certificate failure.
    #[error("tls failure: {0}")]
    Tls(String),
    /// I/O failure mid-body.
    #[error("io failure: {0}")]
    Io(String),
    /// The peer violated the response framing contract.
    #[error("protocol violation: {0}")]
    Protocol(String),
}

/// Sends [`TransportRequest`]s and returns [`TransportResponse`]s. Object-safe
/// so adapters hold `Arc<dyn Transport>`; production wiring is the
/// feature-gated reqwest transport, tests inject in-memory impls.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Performs one request. `Err` covers transport-level failures only;
    /// HTTP error statuses arrive as `Ok` responses for the adapter to map.
    async fn send(&self, request: TransportRequest) -> Result<TransportResponse, TransportFailure>;
}
