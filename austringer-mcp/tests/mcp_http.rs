//! T29 Streamable HTTP interop: our reqwest-0.12 backend
//! (`ReqwestSseClient`) against rmcp's OWN `StreamableHttpService` over a
//! real local HTTP server (axum) — the server side is rmcp's tower
//! service, the client side is ours; SSE decoding, session lifecycle,
//! initialize handshake, tools/list, tools/call, and resources all cross
//! the real wire.

use austringer_mcp::client::McpClient;
use austringer_mcp::config::{ServerEntry, TransportSpec};
use austringer_mcp::reference_server::ReferenceFsServer;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
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
