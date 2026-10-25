//! T29 client interop: in-process duplex wire against the rmcp server
//! side (genuine protocol-level interop), roots escape denial on the live
//! path, typed unresponsive timeouts (non-stalling), resources/prompts
//! round-trips, a REAL child-process stdio run against the
//! `mcp-reference-server` bin, and the stderr bridge.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use austringer_mcp::client::McpClient;
use austringer_mcp::config::{ServerEntry, TransportSpec};
use austringer_mcp::error::McpClientError;
use austringer_mcp::reference_server::{ReferenceFsServer, write_fixture};
use common::duplex_wire;
use serde_json::json;
use tempfile::TempDir;

fn fixture_entry(root: &TempDir, name: &str) -> ServerEntry {
    let mut entry = ServerEntry::new(
        name.to_owned(),
        TransportSpec::Stdio {
            command: "unused".to_owned(),
            args: vec![],
        },
    );
    entry.roots.push(root.path().to_path_buf());
    entry
}

#[tokio::test]
async fn list_and_call_over_in_process_wire() {
    let dir = TempDir::new().expect("temp dir");
    write_fixture(dir.path(), "notes/alpha.txt", "protocol-level content");
    let wire = duplex_wire(ReferenceFsServer::new(dir.path().to_path_buf())).await;
    let client = McpClient::connect_with_transport(fixture_entry(&dir, "fs"), wire)
        .await
        .expect("connect over duplex");

    let tools = client.list_tools().await.expect("list tools");
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert!(names.contains(&"read_file"), "tools: {names:?}");

    let result = client
        .call_tool("read_file", &json!({ "path": "notes/alpha.txt" }))
        .await
        .expect("call read_file");
    assert_eq!(result, b"protocol-level content");
    client.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn root_escape_attempt_denied_over_wire() {
    let inside = TempDir::new().expect("inside dir");
    let outside = TempDir::new().expect("outside dir");
    let escapee = write_fixture(outside.path(), "secret.txt", "must never be read");
    write_fixture(outside.path(), "notes/x.txt", "x");

    let mut entry = fixture_entry(&inside, "fs");
    entry.roots.clear();
    entry.roots.push(inside.path().to_path_buf());
    let wire = duplex_wire(ReferenceFsServer::new(outside.path().to_path_buf())).await;
    let client = McpClient::connect_with_transport(entry, wire)
        .await
        .expect("connect");

    let arguments = json!({ "path": escapee.to_string_lossy() });
    let denial = client
        .call_tool("read_file", &arguments)
        .await
        .expect_err("escape must be denied");
    assert!(
        matches!(denial, McpClientError::RootsEscape { ref path, .. } if path.contains("secret.txt")),
        "got {denial:?}"
    );
    let in_root = client
        .call_tool("read_file", &json!({ "path": "notes/x.txt" }))
        .await
        .expect("in-root relative path still works");
    assert_eq!(in_root, b"x");
}

#[tokio::test]
async fn unresponsive_server_is_typed_and_non_stalling() {
    let dir = TempDir::new().expect("temp dir");
    write_fixture(dir.path(), "a.txt", "a");
    let wire = duplex_wire(ReferenceFsServer::new(dir.path().to_path_buf())).await;
    let client = McpClient::connect_with_transport(fixture_entry(&dir, "fs"), wire)
        .await
        .expect("connect")
        .with_call_timeout(Duration::from_millis(250));

    let started = std::time::Instant::now();
    let outcome = client.call_tool("hang", &json!({})).await;
    let elapsed = started.elapsed();
    let error = outcome.expect_err("hang must time out");
    assert!(
        matches!(error, McpClientError::Unresponsive { ref server, timeout_ms } if server == "fs" && timeout_ms == 250),
        "got {error:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "call must not stall: {elapsed:?}"
    );
    // Poisoned: the very next call refuses immediately (never a hung await).
    let again = client.call_tool("echo", &json!({ "message": "hi" })).await;
    assert!(matches!(again, Err(McpClientError::Unresponsive { .. })));
}

#[tokio::test]
async fn resources_and_prompts_round_trip() {
    let dir = TempDir::new().expect("temp dir");
    let wire = duplex_wire(ReferenceFsServer::new(dir.path().to_path_buf())).await;
    let client = McpClient::connect_with_transport(fixture_entry(&dir, "fs"), wire)
        .await
        .expect("connect");

    let resources = client.list_resources().await.expect("list resources");
    assert_eq!(
        resources
            .iter()
            .find(|r| r.uri == "fixture://notes")
            .map(|r| r.name.as_str()),
        Some("notes")
    );
    let body = client
        .read_resource("fixture://notes")
        .await
        .expect("read resource");
    assert_eq!(body, b"fixture resource body");

    let prompts = client.list_prompts().await.expect("list prompts");
    assert_eq!(prompts.first().map(|p| p.name.as_str()), Some("greet"));
    let rendered = client.get_prompt("greet").await.expect("get prompt");
    assert_eq!(rendered, vec!["hello from the reference prompt".to_owned()]);
}

#[tokio::test]
async fn connect_timeout_is_enforced_typed() {
    let entry = ServerEntry {
        connect_timeout_ms: 150,
        transport: TransportSpec::Http {
            url: "http://127.0.0.1:9/mcp".to_owned(),
        },
        ..fixture_entry(&TempDir::new().expect("temp"), "dead")
    };
    let outcome = McpClient::connect(entry).await;
    match outcome {
        Err(McpClientError::ConnectTimeout {
            ref server,
            timeout_ms,
        }) if server == "dead" && timeout_ms == 150 => {}
        other => panic!("expected typed ConnectTimeout, got {:?}", other.map(|_| ())),
    }
}

/// REAL stdio child interop against the shipped `mcp-reference-server`
/// bin: exercises the spawn path, the stderr bridge, and the full stdio
/// transport end-to-end.
#[tokio::test]
async fn stdio_child_process_interop() {
    let dir = TempDir::new().expect("temp dir");
    write_fixture(dir.path(), "child.txt", "from a real child process");
    let mut entry = ServerEntry::new(
        "stdio-fixture".to_owned(),
        TransportSpec::Stdio {
            command: env!("CARGO_BIN_EXE_mcp_reference_server").to_owned(),
            args: vec![],
        },
    );
    entry.env.insert(
        "austringer_T29_FS_ROOT".to_owned(),
        dir.path().to_string_lossy().into_owned(),
    );
    entry.roots.push(dir.path().to_path_buf());

    let client = McpClient::connect(entry).await.expect("spawn + connect");
    let tools = client.list_tools().await.expect("list tools");
    assert!(tools.iter().any(|tool| tool.name == "read_file"));
    let result = client
        .call_tool("read_file", &json!({ "path": "child.txt" }))
        .await
        .expect("call over real stdio");
    assert_eq!(result, b"from a real child process");
    client.shutdown().await.expect("graceful shutdown");
}

#[tokio::test]
async fn stderr_bridge_bounds_lines_and_length() {
    // 128 KiB buffer: the 20 KiB hostile line must fit or write_all
    // deadlocks against an unread duplex.
    let (writer, reader) = tokio::io::duplex(128 * 1024);
    let (mut writer, mut reader) = (writer, reader);
    writer
        .write_all(b"first line\nsecond line\n")
        .await
        .expect("seed stderr");
    let huge = "x".repeat(20 * 1024);
    writer
        .write_all(format!("{huge}\n").as_bytes())
        .await
        .expect("huge line");
    drop(writer);

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let total = austringer_mcp::stderr_bridge::drain_stderr(&mut reader, move |line| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(line.to_owned());
        }
    })
    .await;
    let lines = seen
        .lock()
        .map(|guard| guard.clone())
        .ok()
        .unwrap_or_default();
    assert_eq!(total, 3, "all three lines counted");
    assert_eq!(lines.len(), 3, "nothing silently dropped");
    assert_eq!(lines[0], "first line");
    let truncated = &lines[2];
    assert!(
        truncated.contains("(truncated,"),
        "long line bounded: len {}",
        truncated.len()
    );
    assert!(truncated.len() < 12 * 1024);
}

use tokio::io::AsyncWriteExt as _;
