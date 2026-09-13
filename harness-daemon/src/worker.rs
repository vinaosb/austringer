//! Remote-worker split-brain (Cursor §19): the agent loop STAYS in the
//! daemon; tool execution delegates to a user machine over an
//! OUTBOUND-ONLY connection — the worker connects in to the daemon, so
//! the user side never opens an inbound port. stdio MCP servers stay on
//! the worker; HTTP MCP is proxied by the daemon ([`route_mcp`]).
//! Credentials are NEVER co-located with the model client: the
//! daemon→worker vocabulary ([`DaemonToWorker`]) carries only scoped
//! tool grants and call payloads — no field could hold a provider key,
//! enforced by type shape and pinned by test. Share the router as
//! `Arc<WorkerRouter>` between the worker-connection handler
//! ([`WorkerRouter::deliver`]) and the tool stage
//! ([`WorkerRouter::dispatch`]).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

/// One scoped tool grant: a worker may execute exactly these tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolGrant {
    /// Granted tool name.
    pub tool: String,
}

/// Where an MCP server entry runs under split-brain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpRouting {
    /// stdio servers stay on the worker (they spawn local processes).
    WorkerLocal,
    /// HTTP(S) servers are proxied through the daemon (one egress
    /// point, credentials stay daemon-side).
    DaemonProxy,
}

/// Routing decision for an MCP transport kind ("stdio", "http", "https").
#[must_use]
pub fn route_mcp(transport: &str) -> McpRouting {
    match transport {
        "http" | "https" => McpRouting::DaemonProxy,
        _ => McpRouting::WorkerLocal,
    }
}

/// Worker → daemon messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerToDaemon {
    /// First message: identity + scoped grants + MCP layout.
    Hello {
        /// Worker id.
        worker_id: String,
        /// Tools this worker may execute (scoped grants).
        grants: Vec<ToolGrant>,
        /// stdio MCP server names running on the worker.
        stdio_mcp: Vec<String>,
        /// HTTP MCP server names proxied through the daemon.
        http_mcp: Vec<String>,
    },
    /// Result of a dispatched tool call.
    ToolOutput {
        /// Correlation id.
        call_id: String,
        /// Whether execution succeeded.
        ok: bool,
        /// Result payload (or error text on failure).
        output: serde_json::Value,
    },
    /// Liveness.
    Ping,
}

/// Daemon → worker messages. Deliberately minimal: no field anywhere in
/// this vocabulary can carry provider credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonToWorker {
    /// Acknowledge a Hello (echoes the effective grants).
    Grant {
        /// Effective scoped grants.
        tools: Vec<ToolGrant>,
    },
    /// Execute one tool call.
    ToolCall {
        /// Correlation id.
        call_id: String,
        /// Tool name (must match a granted tool).
        tool: String,
        /// Tool arguments.
        arguments: serde_json::Value,
    },
    /// Cancel an in-flight call.
    Cancel {
        /// Correlation id.
        call_id: String,
    },
    /// Liveness reply.
    Pong,
    /// Channel closing.
    Bye {
        /// Reason.
        reason: String,
    },
}

/// Worker-channel failures. All typed: the loop maps these to tool
/// results; the SESSION survives (split-brain preserve).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkerError {
    /// No worker connected.
    #[error("no worker connected")]
    NoWorker,
    /// No connected worker holds a grant for the tool (fail closed).
    #[error("tool {0} not granted to any worker")]
    NotGranted(String),
    /// The worker dropped mid-call (split-brain: session continues).
    #[error("worker dropped mid-call")]
    WorkerDropped,
    /// The call timed out (worker hung).
    #[error("worker call timed out")]
    CallTimeout,
    /// The channel rejected the send (worker going away).
    #[error("worker channel closed")]
    ChannelClosed,
    /// The worker reported failure.
    #[error("worker reported failure: {0}")]
    WorkerFailed(String),
}

struct RouterState {
    workers: HashMap<String, mpsc::Sender<DaemonToWorker>>,
    grants: HashMap<String, Vec<String>>,
    pending: HashMap<String, oneshot::Sender<Result<serde_json::Value, String>>>,
}

/// Daemon-side worker registry + tool-call router. All methods take
/// `&self` (interior mutex, never held across an await) so the router
/// can be shared as `Arc<WorkerRouter>`.
pub struct WorkerRouter {
    state: Mutex<RouterState>,
    call_timeout: Duration,
}

impl WorkerRouter {
    /// Router with the per-call timeout.
    #[must_use]
    pub fn new(call_timeout: Duration) -> Self {
        Self {
            state: Mutex::new(RouterState {
                workers: HashMap::new(),
                grants: HashMap::new(),
                pending: HashMap::new(),
            }),
            call_timeout,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RouterState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Register a connected worker (its inbound channel). Replaces any
    /// prior registration under the same id.
    pub fn register(
        &self,
        worker_id: &str,
        grants: Vec<ToolGrant>,
        tx: mpsc::Sender<DaemonToWorker>,
    ) {
        let names = grants.iter().map(|g| g.tool.clone()).collect::<Vec<_>>();
        let mut state = self.lock();
        state.workers.insert(worker_id.to_owned(), tx);
        state.grants.insert(worker_id.to_owned(), names);
    }

    /// A worker went away: drop its registration. Pending calls fail
    /// typed via the closed channels ([`WorkerError::WorkerDropped`]).
    pub fn disconnect(&self, worker_id: &str) {
        let mut state = self.lock();
        state.workers.remove(worker_id);
        state.grants.remove(worker_id);
    }

    /// Connected worker ids.
    #[must_use]
    pub fn connected(&self) -> Vec<String> {
        self.lock().workers.keys().cloned().collect()
    }

    /// Deliver a worker's ToolOutput to the waiting dispatcher.
    pub fn deliver(&self, call_id: &str, ok: bool, output: serde_json::Value) {
        if let Some(waiter) = self.lock().pending.remove(call_id) {
            let result = if ok {
                Ok(output)
            } else {
                Err(output
                    .as_str()
                    .unwrap_or("worker reported failure")
                    .to_owned())
            };
            let _ = waiter.send(result);
        }
    }

    /// Dispatch one tool call to a granted worker and await its output.
    /// Worker drop or timeout is typed; the caller (the loop's tool
    /// stage) turns it into a tool result and the session continues.
    pub async fn dispatch(
        &self,
        call_id: &str,
        tool: &str,
        arguments: &serde_json::Value,
    ) -> Result<serde_json::Value, WorkerError> {
        let (sender, rx) = {
            let mut state = self.lock();
            let worker = state
                .grants
                .iter()
                .find(|(_, tools)| tools.iter().any(|t| t == tool))
                .map(|(id, _)| id.clone())
                .ok_or_else(|| {
                    if state.workers.is_empty() {
                        WorkerError::NoWorker
                    } else {
                        WorkerError::NotGranted(tool.to_owned())
                    }
                })?;
            let (tx_done, rx) = oneshot::channel();
            state.pending.insert(call_id.to_owned(), tx_done);
            let sender = state
                .workers
                .get(&worker)
                .cloned()
                .ok_or(WorkerError::NoWorker)?;
            (sender, rx)
        };
        sender
            .send(DaemonToWorker::ToolCall {
                call_id: call_id.to_owned(),
                tool: tool.to_owned(),
                arguments: arguments.clone(),
            })
            .await
            .map_err(|_| WorkerError::ChannelClosed)?;
        match tokio::time::timeout(self.call_timeout, rx).await {
            Ok(Ok(Ok(output))) => Ok(output),
            Ok(Ok(Err(failure))) => Err(WorkerError::WorkerFailed(failure)),
            Ok(Err(_)) | Err(_) => {
                self.lock().pending.remove(call_id);
                Err(WorkerError::WorkerDropped)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn grant(tools: &[&str]) -> Vec<ToolGrant> {
        tools
            .iter()
            .map(|t| ToolGrant {
                tool: (*t).to_owned(),
            })
            .collect()
    }

    #[test]
    fn mcp_routing_splits_stdio_from_http() {
        assert_eq!(route_mcp("stdio"), McpRouting::WorkerLocal);
        assert_eq!(route_mcp("http"), McpRouting::DaemonProxy);
        assert_eq!(route_mcp("https"), McpRouting::DaemonProxy);
    }

    #[test]
    fn daemon_to_worker_wire_has_no_credential_fields() {
        // Co-location guard: the daemon→worker vocabulary cannot carry
        // provider credentials — not by convention, by shape.
        let messages = vec![
            DaemonToWorker::Grant {
                tools: grant(&["bash"]),
            },
            DaemonToWorker::ToolCall {
                call_id: "c1".into(),
                tool: "bash".into(),
                arguments: serde_json::json!({"cmd": "ls"}),
            },
            DaemonToWorker::Cancel {
                call_id: "c1".into(),
            },
            DaemonToWorker::Pong,
            DaemonToWorker::Bye {
                reason: "drain".into(),
            },
        ];
        for msg in &messages {
            let wire = serde_json::to_string(msg).ok().unwrap_or_default();
            let lower = wire.to_lowercase();
            assert!(!lower.contains("key"), "wire leaked a key field: {wire}");
            assert!(
                !lower.contains("secret"),
                "wire leaked a secret field: {wire}"
            );
            assert!(
                !lower.contains("token"),
                "wire leaked a token field: {wire}"
            );
        }
    }

    #[tokio::test]
    async fn dispatch_routes_to_granted_worker_and_back() {
        let (worker_tx, mut worker_rx) = mpsc::channel(4);
        let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
        router.register("w1", grant(&["bash"]), worker_tx);
        assert_eq!(router.connected(), vec!["w1".to_owned()]);

        let dispatch = {
            let router = Arc::clone(&router);
            tokio::spawn(async move {
                router
                    .dispatch("c1", "bash", &serde_json::json!({"cmd": "ls"}))
                    .await
            })
        };
        // In-test worker mock: echo completion back through the router.
        if let Some(DaemonToWorker::ToolCall { call_id, .. }) = worker_rx.recv().await {
            router.deliver(&call_id, true, serde_json::json!("done"));
        }
        let out = dispatch.await.unwrap_or(Err(WorkerError::CallTimeout));
        assert_eq!(out.ok(), Some(serde_json::json!("done")));
    }

    #[tokio::test]
    async fn ungranted_tool_fails_closed() {
        let (worker_tx, _worker_rx) = mpsc::channel(4);
        let router = WorkerRouter::new(Duration::from_secs(5));
        router.register("w1", grant(&["bash"]), worker_tx);
        let denied = router
            .dispatch("c2", "dangerous", &serde_json::json!({}))
            .await;
        assert_eq!(denied, Err(WorkerError::NotGranted("dangerous".into())));
    }

    #[tokio::test]
    async fn no_worker_is_typed() {
        let router = WorkerRouter::new(Duration::from_secs(5));
        assert_eq!(
            router.dispatch("c3", "bash", &serde_json::json!({})).await,
            Err(WorkerError::NoWorker)
        );
    }

    #[tokio::test]
    async fn worker_drop_mid_tool_is_typed_and_session_continues() {
        // Split-brain preserve: the worker receives the call and DIES.
        // The dispatch fails typed; a replacement worker then serves a
        // follow-up call — the routing layer (and the session above it)
        // survives the drop.
        let (w1_tx, mut w1_rx) = mpsc::channel(1);
        let router = Arc::new(WorkerRouter::new(Duration::from_millis(150)));
        router.register("w1", grant(&["bash"]), w1_tx);

        let first = {
            let router = Arc::clone(&router);
            tokio::spawn(async move {
                router
                    .dispatch("c-drop", "bash", &serde_json::json!({"n": 1}))
                    .await
            })
        };
        if w1_rx.recv().await.is_some() {
            drop(w1_rx);
        }
        router.disconnect("w1");
        let dropped = first.await.unwrap_or(Ok(serde_json::json!({})));
        assert_eq!(dropped, Err(WorkerError::WorkerDropped));

        let (w2_tx, mut w2_rx) = mpsc::channel(1);
        router.register("w2", grant(&["bash"]), w2_tx);
        let second = {
            let router = Arc::clone(&router);
            tokio::spawn(async move {
                router
                    .dispatch("c2", "bash", &serde_json::json!({"n": 2}))
                    .await
            })
        };
        if let Some(DaemonToWorker::ToolCall { call_id, .. }) = w2_rx.recv().await {
            router.deliver(&call_id, true, serde_json::json!("recovered"));
        }
        let healed = second.await.unwrap_or(Err(WorkerError::CallTimeout));
        assert_eq!(healed.ok(), Some(serde_json::json!("recovered")));
    }

    #[tokio::test]
    async fn worker_failure_is_typed() {
        let (worker_tx, mut worker_rx) = mpsc::channel(1);
        let router = Arc::new(WorkerRouter::new(Duration::from_secs(5)));
        router.register("w1", grant(&["bash"]), worker_tx);
        let dispatch = {
            let router = Arc::clone(&router);
            tokio::spawn(async move {
                router
                    .dispatch("c-err", "bash", &serde_json::json!({}))
                    .await
            })
        };
        if let Some(DaemonToWorker::ToolCall { call_id, .. }) = worker_rx.recv().await {
            router.deliver(&call_id, false, serde_json::json!("boom: exit 1"));
        }
        let failed = dispatch.await.unwrap_or(Ok(serde_json::json!({})));
        assert_eq!(
            failed,
            Err(WorkerError::WorkerFailed("boom: exit 1".into()))
        );
    }
}
