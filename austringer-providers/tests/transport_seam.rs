//! Stage-A tests for the hermetic transport seam: request building, streaming
//! bodies, object safety, and the retry classification of transport failures.

use std::sync::Arc;

use async_trait::async_trait;

use austringer_providers::error::ProviderStreamError;
use austringer_providers::transport::{
    Body, ByteSource, ByteStream, Transport, TransportFailure, TransportRequest, TransportResponse,
};

/// In-memory chunk source over owned byte vectors.
struct VecChunks {
    chunks: Vec<Vec<u8>>,
}

#[async_trait]
impl ByteSource for VecChunks {
    async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        if self.chunks.is_empty() {
            None
        } else {
            Some(self.chunks.remove(0))
        }
    }
}

fn stream_of(chunks: Vec<Vec<u8>>) -> ByteStream {
    ByteStream::new(Box::new(VecChunks { chunks }))
}

#[tokio::test]
async fn byte_stream_yields_chunks_in_order_then_ends() {
    let mut stream = stream_of(vec![b"a".to_vec(), b"bc".to_vec()]);
    assert_eq!(stream.next_chunk().await, Some(b"a".to_vec()));
    assert_eq!(stream.next_chunk().await, Some(b"bc".to_vec()));
    assert_eq!(stream.next_chunk().await, None);
}

#[tokio::test]
async fn transport_request_accumulates_headers() {
    let request = TransportRequest::new("https://example.test/v1/x", b"{}".to_vec())
        .with_header("content-type", "application/json")
        .with_header("x-extra", "1");
    assert_eq!(request.url, "https://example.test/v1/x");
    assert_eq!(
        request.headers.get("content-type").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(
        request.headers.get("x-extra").map(String::as_str),
        Some("1")
    );
    assert_eq!(request.body, b"{}".to_vec());
}

/// Smallest possible Transport impl: proves the trait is dyn-safe and that a
/// full-body response round-trips.
struct FullBodyTransport;

#[async_trait]
impl Transport for FullBodyTransport {
    async fn send(
        &self,
        _request: TransportRequest,
    ) -> Result<TransportResponse, TransportFailure> {
        Ok(TransportResponse {
            status: 200,
            body: Body::Full(b"ok".to_vec()),
        })
    }
}

#[tokio::test]
async fn transport_trait_is_dyn_safe_and_full_body_round_trips() {
    let transport: Arc<dyn Transport> = Arc::new(FullBodyTransport);
    let response = transport
        .send(TransportRequest::new("https://example.test", Vec::new()))
        .await
        .expect("scripted transport cannot fail");
    assert_eq!(response.status, 200);
    match response.body {
        Body::Full(bytes) => assert_eq!(bytes, b"ok".to_vec()),
        Body::Stream(_) => panic!("full-body transport returned a stream"),
    }
}

#[test]
fn transport_failures_map_to_retryable_stream_errors() {
    for failure in [
        TransportFailure::Connect("refused".to_owned()),
        TransportFailure::Timeout("deadline".to_owned()),
        TransportFailure::Tls("handshake".to_owned()),
        TransportFailure::Io("reset".to_owned()),
        TransportFailure::Protocol("bad frame".to_owned()),
    ] {
        let mapped = ProviderStreamError::Transport(failure);
        assert!(
            mapped.is_recoverable(),
            "{mapped:?} must classify retryable"
        );
    }
}
