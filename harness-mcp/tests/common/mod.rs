//! Shared test plumbing: the in-process duplex wire. The reference
//! server itself lives in the library (`harness_mcp::reference_server`)
//! so the `mcp-reference-server` bin can serve it over real stdio.

#![allow(dead_code)]

/// Serves one in-test server on one end of a tokio duplex, as a TASK
/// (`serve_server` blocks until the client's initialize arrives — running
/// it inline deadlocks), keeping the service alive until the wire closes.
/// Returns the other end as the client transport (keep the test's client
/// alive for the connection's lifetime).
pub async fn duplex_wire<S: rmcp::ServerHandler + 'static>(server: S) -> tokio::io::DuplexStream {
    let (client_end, server_end) = tokio::io::duplex(8192);
    tokio::spawn(async move {
        if let Ok(service) = rmcp::service::serve_server(server, server_end).await {
            let _ = service.waiting().await;
        }
    });
    client_end
}
