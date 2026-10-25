//! ServerHandle + handshake lifecycle against the in-test mock LS:
//! initialize negotiation, request/response routing, server-initiated
//! requests answered with null, stderr capture, hung-request timeouts,
//! transport death, mid-flight cancellation, garbage frames, and the
//! graceful shutdown/exit sequence.

use std::sync::Arc;
use std::time::Duration;

use austringer_lsp::config::Language;
use austringer_lsp::error::LspError;
use austringer_lsp::handshake::{initialize, initialized, shutdown_graceful};
use austringer_lsp::native::{PositionEncoding, ServerHandle};
use austringer_lsp::uri::file_to_uri;
use common::{Action, Handler, MockLsp, spawn_mock, utf8_caps};
use serde_json::{Value, json};
use tempfile::TempDir;

mod common;

fn always_ok() -> Handler {
    Arc::new(|method: &str, _params: &Value| match method {
        "initialize" => Action::Result(utf8_caps()),
        _ => Action::Result(Value::Null),
    })
}

async fn attached(handler: Handler) -> (ServerHandle, MockLsp) {
    let ((read, write), mock) = spawn_mock(handler).await;
    (ServerHandle::attach(read, write, None), mock)
}

#[tokio::test]
async fn initialize_negotiates_utf8_and_carries_root_uri() {
    let root = TempDir::new().expect("tempdir");
    let (handle, mut mock) = attached(always_ok()).await;

    let encoding = initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("initialize succeeds");
    assert_eq!(encoding, PositionEncoding::Utf8);

    let msg = mock.wait_for_method("initialize").await;
    let params = msg.get("params").expect("initialize has params");
    assert_eq!(
        params.get("rootUri").and_then(Value::as_str),
        file_to_uri(root.path()).as_deref(),
        "rootUri must be the encoded workspace root"
    );
    assert!(
        params
            .get("capabilities")
            .and_then(|c| c.get("general"))
            .is_some(),
        "client capabilities must be present: {params}"
    );
    initialized(&handle).await.expect("initialized notify");
    mock.wait_for_method("initialized").await;
}

#[tokio::test]
async fn utf16_only_server_is_detected() {
    let root = TempDir::new().expect("tempdir");
    let (handle, _mock) = attached(Arc::new(|_m: &str, _p: &Value| {
        Action::Result(common::utf16_caps())
    }))
    .await;

    let encoding = initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("initialize succeeds");
    assert_eq!(encoding, PositionEncoding::Utf16);
}

#[tokio::test]
async fn hung_initialize_is_a_typed_timeout() {
    let root = TempDir::new().expect("tempdir");
    let (handle, _mock) = attached(Arc::new(|_m: &str, _p: &Value| Action::Hang)).await;

    let err = initialize(
        &handle,
        root.path(),
        Language::Rust,
        Duration::from_millis(75),
    )
    .await
    .expect_err("hung server must time out");
    assert!(
        matches!(err, LspError::InitializeTimeout { language: Language::Rust, .. } if waited_matches(&err)),
        "expected typed InitializeTimeout, got: {err}"
    );
}

fn waited_matches(err: &LspError) -> bool {
    matches!(err, LspError::InitializeTimeout { waited, .. } if *waited == Duration::from_millis(75))
}

#[tokio::test]
async fn requests_round_trip_and_map_error_objects() {
    let root = TempDir::new().expect("tempdir");
    let (handle, mut mock) = attached(Arc::new(|method: &str, _p: &Value| match method {
        "initialize" => Action::Result(utf8_caps()),
        "textDocument/definition" => Action::Result(json!([{
            "uri": "file:///C:/w/src/lib.rs",
            "range": {"start": {"line": 3, "character": 7},
                      "end": {"line": 3, "character": 7}}
        }])),
        "textDocument/references" => Action::Error(-32601),
        _ => Action::Result(Value::Null),
    }))
    .await;
    initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("init");

    let ok = handle
        .request(
            "textDocument/definition",
            json!({"textDocument": {"uri": "file:///C:/w/src/lib.rs"},
                   "position": {"line": 0, "character": 0}}),
            Duration::from_secs(5),
        )
        .await
        .expect("definition request");
    assert!(
        ok.get(0).and_then(|l| l.get("uri")).is_some(),
        "payload: {ok}"
    );

    let err = handle
        .request("textDocument/references", json!({}), Duration::from_secs(5))
        .await
        .expect_err("error object must propagate typed");
    assert!(
        matches!(err, LspError::RequestFailed { code: -32601, .. }),
        "expected RequestFailed -32601, got: {err}"
    );
    mock.wait_for_method("textDocument/definition").await;
}

#[tokio::test]
async fn server_initiated_requests_get_null_results() {
    let root = TempDir::new().expect("tempdir");
    let (handle, mut mock) = attached(always_ok()).await;
    initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("init");

    mock.send(&json!({"jsonrpc": "2.0", "id": "srv-1",
                       "method": "workspace/configuration", "params": {}}))
        .await;
    let reply = mock.wait_for_response(&json!("srv-1")).await;
    assert_eq!(reply.get("result"), Some(&Value::Null));
}

#[tokio::test]
async fn stderr_is_captured_never_printed() {
    let ((client_read, client_write), mock) = spawn_mock(always_ok()).await;
    let (stderr_client, mut stderr_server) = tokio::io::duplex(1024);
    let handle = ServerHandle::attach(client_read, client_write, Some(Box::new(stderr_client)));

    use tokio::io::AsyncWriteExt;
    stderr_server
        .write_all(b"spewing diagnostics\nline two\n")
        .await
        .expect("stderr write");
    stderr_server.flush().await.expect("stderr flush");

    let tail = handle.wait_stderr_tail(Duration::from_secs(5)).await;
    assert!(
        tail.iter().any(|l| l.contains("spewing diagnostics")),
        "captured tail: {tail:?}"
    );
    assert!(tail.len() <= 8, "ring buffer stays bounded: {tail:?}");
    let _ = stderr_server;
    let _ = mock;
    let _ = handle;
}

#[tokio::test]
async fn hung_request_is_a_typed_timeout_and_slot_stays_usable() {
    let root = TempDir::new().expect("tempdir");
    let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let gate_reader = Arc::clone(&gate);
    let (handle, _mock) = attached(Arc::new(move |method: &str, _p: &Value| match method {
        "initialize" => Action::Result(utf8_caps()),
        _ if gate_reader.load(std::sync::atomic::Ordering::SeqCst) => Action::Result(Value::Null),
        _ => Action::Hang,
    }))
    .await;
    initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("init");

    let err = handle
        .request(
            "textDocument/definition",
            json!({}),
            Duration::from_millis(75),
        )
        .await
        .expect_err("hung request must time out");
    assert!(
        matches!(
            err,
            LspError::RequestTimeout {
                method: "textDocument/definition",
                ..
            }
        ),
        "expected RequestTimeout, got: {err}"
    );

    gate.store(true, std::sync::atomic::Ordering::SeqCst);
    handle
        .request("textDocument/definition", json!({}), Duration::from_secs(5))
        .await
        .expect("server is still usable after a request timeout");
}

#[tokio::test]
async fn cancelled_request_is_cleanly_dropped_and_server_survives() {
    let root = TempDir::new().expect("tempdir");
    let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let gate_reader = Arc::clone(&gate);
    let (handle, mut mock) = attached(Arc::new(move |method: &str, _p: &Value| match method {
        "initialize" => Action::Result(utf8_caps()),
        _ if gate_reader.load(std::sync::atomic::Ordering::SeqCst) => Action::Result(Value::Null),
        _ => Action::Hang,
    }))
    .await;
    initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("init");

    let mut in_flight = std::pin::pin!(handle.request(
        "textDocument/definition",
        json!({"marker": "cancelled-one"}),
        Duration::from_secs(60),
    ));
    // The handler hangs, so the request parks once it is on the wire;
    // abandon it mid-flight right after the mock confirms receipt.
    let abandoned = tokio::select! {
        outcome = &mut in_flight => Some(outcome),
        _ = tokio::time::sleep(Duration::from_millis(50)) => None,
    };
    assert!(
        abandoned.is_none(),
        "future must still be pending when dropped"
    );
    let seen = mock.wait_for_method("textDocument/definition").await;
    assert_eq!(
        seen.get("params").and_then(|p| p.get("marker")),
        Some(&json!("cancelled-one")),
        "the cancelled request must have reached the server"
    );

    gate.store(true, std::sync::atomic::Ordering::SeqCst);
    handle
        .request("textDocument/definition", json!({}), Duration::from_secs(5))
        .await
        .expect("clean drop: next request succeeds on the same connection");
}

#[tokio::test]
async fn transport_death_fails_the_pending_request() {
    let root = TempDir::new().expect("tempdir");
    let (handle, mut mock) = attached(Arc::new(|method: &str, _p: &Value| match method {
        "initialize" => Action::Result(utf8_caps()),
        _ => Action::Hang,
    }))
    .await;
    initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("init");

    let mut request = std::pin::pin!(handle.request(
        "textDocument/definition",
        json!({}),
        Duration::from_secs(60),
    ));
    // Drive the request onto the wire concurrently with waiting for the
    // mock's receipt confirmation (the handler hangs, so the request stays
    // pending for the whole window).
    let early = {
        let confirm = mock.wait_for_method("textDocument/definition");
        tokio::pin!(confirm);
        tokio::select! {
            _ = &mut confirm => None,
            result = &mut request => Some(result),
        }
    };
    assert!(
        early.is_none(),
        "handler hangs, so the request must still be pending"
    );

    mock.crash();
    let result = request.await;
    assert!(
        result.is_err(),
        "pending request must fail when the server dies"
    );
    assert!(
        matches!(
            result.expect_err("checked above"),
            LspError::Io(_) | LspError::MalformedFrame { .. }
        ),
        "death surfaces as transport-class error"
    );
}

#[tokio::test]
async fn garbage_frames_fail_typed_without_oom() {
    let root = TempDir::new().expect("tempdir");
    let (handle, mock) =
        attached(Arc::new(|_m: &str, _p: &Value| Action::Result(utf8_caps()))).await;
    initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("init");

    // A lying Content-Length plus binary garbage where a frame should be.
    mock.send_raw(b"Content-Length: 999999999\r\n\r\n\x00\x01\x02garbage")
        .await;

    let err = handle
        .request("textDocument/definition", json!({}), Duration::from_secs(5))
        .await
        .expect_err("corrupt stream must fail the request");
    assert!(
        matches!(err, LspError::MalformedFrame { .. }),
        "expected typed MalformedFrame, got: {err}"
    );
    assert!(handle.is_dead(), "reader task must mark the handle dead");
}

#[tokio::test]
async fn graceful_shutdown_sends_shutdown_then_exit() {
    let root = TempDir::new().expect("tempdir");
    let (handle, mut mock) = attached(always_ok()).await;
    initialize(&handle, root.path(), Language::Rust, Duration::from_secs(5))
        .await
        .expect("init");

    shutdown_graceful(&handle, Duration::from_secs(5))
        .await
        .expect("graceful shutdown");
    let shutdown = mock.wait_for_method("shutdown").await;
    assert!(shutdown.get("id").is_some(), "shutdown must be a request");
    mock.wait_for_method("exit").await;
    // Dropping the handle kills the client transport (Notify path), but the
    // mock's own serve task only ends via its crash switch — the analog of
    // the server process exiting its main loop.
    drop(handle);
    mock.crash();
    mock.join().await;
}
