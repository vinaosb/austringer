//! The LSP lifecycle: `initialize` handshake with capability negotiation,
//! the `initialized` notification, and the graceful `shutdown`/`exit`
//! pair (with a hard-kill fallback).

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::timeout;

use crate::config::Language;
use crate::error::LspError;
use crate::native::{PositionEncoding, ServerHandle};
use crate::uri::file_to_uri;

/// Runs the `initialize` request/response and returns the negotiated
/// position encoding. The client advertises UTF-8 positions first
/// (`general.positionEncodings`); servers that do not honor it keep the
/// LSP default (UTF-16 code units), which the caller must convert.
///
/// On timeout the server process is killed (hung handshakes never leave a
/// zombie) and [`LspError::InitializeTimeout`] is returned.
pub async fn initialize(
    handle: &ServerHandle,
    root: &Path,
    language: Language,
    initialize_timeout: Duration,
) -> Result<PositionEncoding, LspError> {
    let Some(root_uri) = file_to_uri(root) else {
        return Err(LspError::NoServer {
            detail: format!("workspace root {} is not valid UTF-8", root.display()),
        });
    };
    let params = json!({
        "processId": std::process::id(),
        "rootUri": root_uri,
        "workspaceFolders": [{"uri": root_uri, "name": root.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "root".to_owned())}],
        "clientInfo": {"name": "austringer-lsp", "version": env!("CARGO_PKG_VERSION")},
        "capabilities": {
            "general": {"positionEncodings": ["utf-8", "utf-16"]},
            "textDocument": {"synchronization": {"dynamicRegistration": false}}
        }
    });

    // The wire request carries a doubled budget so the OUTER deadline here
    // owns the failure mode: a hung initialize surfaces as
    // InitializeTimeout (with the server killed), not RequestTimeout.
    let wire_budget = initialize_timeout.saturating_mul(2);
    let result = match timeout(
        initialize_timeout,
        handle.request("initialize", params, wire_budget),
    )
    .await
    {
        Ok(outcome) => outcome?,
        Err(_elapsed) => {
            handle.kill();
            return Err(LspError::InitializeTimeout {
                language,
                waited: initialize_timeout,
            });
        }
    };

    let negotiated = result
        .get("capabilities")
        .and_then(|c| c.get("positionEncoding"))
        .and_then(Value::as_str);
    Ok(match negotiated {
        Some("utf-8") => PositionEncoding::Utf8,
        _ => PositionEncoding::Utf16,
    })
}

/// Sends the `initialized` notification that completes the handshake.
pub async fn initialized(handle: &ServerHandle) -> Result<(), LspError> {
    handle.notify("initialized", json!({})).await
}

/// Graceful lifecycle end: `shutdown` request, `exit` notification, then
/// wait (bounded) for the process to leave. Falls back to a hard kill —
/// callers never wait forever on a server that ignores the protocol.
pub async fn shutdown_graceful(
    handle: &ServerHandle,
    wait_timeout: Duration,
) -> Result<(), LspError> {
    let _ = timeout(
        wait_timeout,
        handle.request("shutdown", json!(null), wait_timeout),
    )
    .await;
    handle.notify("exit", json!(null)).await?;
    handle.kill();
    Ok(())
}
