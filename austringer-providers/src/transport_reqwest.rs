//! Production [`Transport`] over reqwest (rustls, no default features —
//! see the workspace manifest for the feature-set rationale).
//!
//! All provider requests are `POST`s; responses are ALWAYS returned as
//! streaming bodies — the adapter layer owns bounded reads and SSE framing,
//! so error payloads and auth bodies flow through the same 64 KiB-bounded
//! path. A mid-body transport error surfaces as end-of-body plus a warn
//! log: the turn layer's complete-or-error contract turns that into a
//! typed disconnect and the resume machinery takes over, which is exactly
//! the recovery semantics a dropped connection deserves.
//!
//! This impl is never exercised by the test suite (house rule: zero network
//! in tests); its correctness rests on the seam contract shared with the
//! in-memory transports that ARE tested.

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt as _;

use crate::transport::{
    Body, ByteSource, ByteStream, Transport, TransportFailure, TransportRequest, TransportResponse,
};

/// HTTP transport over a rustls-backed reqwest client.
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Builds a client with the default 15 s connect timeout and no overall
    /// request timeout (SSE streams run long; liveness is the turn layer's
    /// idle budget).
    ///
    /// # Errors
    /// [`TransportFailure::Tls`] when the TLS backend fails to initialize.
    pub fn new() -> Result<Self, TransportFailure> {
        Self::with_connect_timeout(Duration::from_secs(15))
    }

    /// Builds a client with an explicit connect timeout.
    ///
    /// # Errors
    /// [`TransportFailure::Tls`] when the TLS backend fails to initialize.
    pub fn with_connect_timeout(connect_timeout: Duration) -> Result<Self, TransportFailure> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("austringer/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(connect_timeout)
            .build()
            .map_err(|err| TransportFailure::Tls(err.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl Transport for ReqwestTransport {
    async fn send(&self, request: TransportRequest) -> Result<TransportResponse, TransportFailure> {
        let mut builder = self.client.post(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let response = builder.body(request.body).send().await.map_err(classify)?;
        let status = response.status().as_u16();
        let stream = response.bytes_stream().boxed();
        Ok(TransportResponse::with_status(
            status,
            Body::Stream(ByteStream::new(Box::new(ReqwestSource { stream }))),
        ))
    }
}

struct ReqwestSource {
    stream: futures_util::stream::BoxStream<'static, reqwest::Result<bytes::Bytes>>,
}

#[async_trait]
impl ByteSource for ReqwestSource {
    async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        match self.stream.next().await {
            Some(Ok(bytes)) => Some(bytes.to_vec()),
            Some(Err(err)) => {
                tracing::warn!(error = %err, "response body failed mid-stream; treated as disconnect");
                None
            }
            None => None,
        }
    }
}

/// reqwest error classes are coarse; the retry decision only needs the
/// transport class (timeout vs connect vs everything-else-as-I/O).
fn classify(err: reqwest::Error) -> TransportFailure {
    if err.is_timeout() {
        TransportFailure::Timeout(err.to_string())
    } else if err.is_connect() {
        TransportFailure::Connect(err.to_string())
    } else {
        TransportFailure::Io(err.to_string())
    }
}
