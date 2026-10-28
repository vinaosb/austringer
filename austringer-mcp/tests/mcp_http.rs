//! T29 Streamable HTTP interop: our reqwest-0.12 backend
//! (`ReqwestSseClient`) against rmcp's OWN `StreamableHttpService` over a
//! real local HTTP server (axum) — the server side is rmcp's tower
//! service, the client side is ours; SSE decoding, session lifecycle,
//! initialize handshake, tools/list, tools/call, and resources all cross
//! the real wire.

use austringer_mcp::client::McpClient;
use austringer_mcp::client::connect_retry_delay;
use austringer_mcp::config::{ServerEntry, TransportSpec};
use austringer_mcp::error::McpClientError;
use austringer_mcp::reference_server::ReferenceFsServer;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tempfile::TempDir;

async fn spawn_http_mcp_server(root: std::path::PathBuf) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let service = StreamableHttpService::new(
        move || Ok::<_, std::io::Error>(ReferenceFsServer::new(root.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}/mcp")
}

#[tokio::test]
async fn streamable_http_full_round_trip() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path().to_path_buf();
    std::fs::write(root.join("http.txt"), "over real http").ok();
    let url = spawn_http_mcp_server(root.clone()).await;
    let mut entry = ServerEntry::new("http-fixture".to_owned(), TransportSpec::Http { url });
    entry.roots.push(root.clone());

    let client = McpClient::connect(entry).await.expect("http connect");
    let tools = client.list_tools().await.expect("list over http");
    assert!(tools.iter().any(|tool| tool.name == "read_file"));

    let result = client
        .call_tool("read_file", &json!({ "path": "http.txt" }))
        .await
        .expect("call over http");
    assert_eq!(result, b"over real http");

    let resources = client.list_resources().await.expect("resources over http");
    assert!(resources.iter().any(|r| r.uri == "fixture://notes"));
    client.shutdown().await.expect("http shutdown");
}

#[tokio::test]
async fn streamable_http_connect_timeout_typed() {
    // Bind then REFUSE to serve MCP: connect must fail typed, fast.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    let entry = ServerEntry {
        connect_timeout_ms: 400,
        transport: TransportSpec::Http {
            url: format!("http://{addr}/mcp"),
        },
        ..ServerEntry::new(
            "refused".to_owned(),
            TransportSpec::Stdio {
                command: String::new(),
                args: vec![],
            },
        )
    };
    let outcome = McpClient::connect(entry).await;
    assert!(outcome.err().is_some(), "refused endpoint must fail");
}

/// A dead-endpoint stand-in: accepts every connection and immediately
/// drops it, counting dials so per-attempt connect behavior is
/// observable without any network egress (loopback only).
async fn spawn_dropping_endpoint() -> (std::net::SocketAddr, Arc<AtomicU64>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let dials = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&dials);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            drop(stream);
            counter.fetch_add(1, Ordering::Relaxed);
        }
    });
    (addr, dials)
}

#[tokio::test]
async fn streamable_http_dead_endpoint_backoff_bounds_dials() {
    // A dead endpoint must see a handful of connect attempts inside the
    // budget, never one per 20 ms: with capped exponential backoff
    // (20 -> 40 -> 80 -> 160 -> 320 ms) retries cannot start before
    // t=0/20/60/140/300, so a 400 ms budget bounds the loop to <= 5
    // dials where a fixed-20 ms loop dialed ~20. The failure must still
    // be the typed ConnectTimeout and must still consume the whole
    // budget (the retry-to-deadline contract).
    let (addr, dials) = spawn_dropping_endpoint().await;
    let entry = ServerEntry {
        connect_timeout_ms: 400,
        transport: TransportSpec::Http {
            url: format!("http://{addr}/mcp"),
        },
        ..ServerEntry::new(
            "dead-endpoint".to_owned(),
            TransportSpec::Stdio {
                command: String::new(),
                args: vec![],
            },
        )
    };
    let started = Instant::now();
    let error = McpClient::connect(entry)
        .await
        .err()
        .expect("dead endpoint must fail");
    let elapsed = started.elapsed();
    assert!(
        matches!(
            error,
            McpClientError::ConnectTimeout {
                timeout_ms: 400,
                ..
            }
        ),
        "retry-to-deadline must surface the typed ConnectTimeout, got {error:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(400),
        "the connect budget must be consumed, elapsed {elapsed:?}"
    );
    let dialed = dials.load(Ordering::Relaxed);
    assert!(
        dialed <= 5,
        "backoff must bound a dead endpoint to <= 5 dials per 400 ms budget, got {dialed}"
    );
}

#[test]
fn streamable_http_connect_retry_delay_is_capped_exponential() {
    // 20 -> 40 -> 80 -> 160 -> 320 -> 500 (640 capped) -> 500 forever;
    // saturating, never overflow-prone even at u32::MAX attempts.
    let expected = [20, 40, 80, 160, 320, 500, 500];
    for (attempt, delay_ms) in expected.iter().enumerate() {
        assert_eq!(
            connect_retry_delay(u32::try_from(attempt).expect("small attempt")),
            Duration::from_millis(*delay_ms),
            "attempt {attempt}"
        );
    }
    assert_eq!(
        connect_retry_delay(u32::MAX),
        Duration::from_millis(500),
        "the delay must saturate at the cap"
    );
}

#[tokio::test]
async fn sse_decoder_rejects_duplicated_field_lines() {
    use futures_util::StreamExt as _;
    use sse_stream::{Error as SseError, Sse};
    let raw = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(bytes::Bytes::from(
        "event: a\nevent: b\ndata: hello\n\n",
    ))]);
    let events: Vec<Result<Sse, SseError>> =
        austringer_mcp::http_backend::decode_sse_for_tests(raw)
            .collect()
            .await;
    assert!(
        events[0]
            .as_ref()
            .is_err_and(|error| matches!(error, SseError::DuplicatedEventLine)),
        "{events:?}"
    );
}

#[tokio::test]
async fn sse_decoder_joins_multiline_data_and_reports_ids() {
    use futures_util::StreamExt as _;
    use sse_stream::Sse;
    let raw = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(bytes::Bytes::from(
        "id: 7\ndata: line one\ndata: line two\n\n: comment only\n\n",
    ))]);
    let events: Vec<Sse> = austringer_mcp::http_backend::decode_sse_for_tests(raw)
        .map(Result::unwrap)
        .collect()
        .await;
    assert_eq!(events.len(), 1, "comment-only frames are skipped");
    assert_eq!(events[0].id.as_deref(), Some("7"));
    assert_eq!(events[0].data.as_deref(), Some("line one\nline two"));
}
