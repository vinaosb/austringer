//! The ACP agent-side core (todo 29 amendment): the harness speaks the
//! Agent Client Protocol v1 (Zed-authored, JetBrains-adopted) so IDE
//! hosts drive it as a panel agent. NDJSON JSON-RPC 2.0 framing over any
//! line transport.
//!
//! NAME-MAPPING DISCLOSURE (plan shorthand → actual v1 spec, verified
//! against schema v1 at `agentclientprotocol/agent-client-protocol`):
//! "initialize" = `initialize` (protocolVersion is an INTEGER, value 1);
//! "edit-request" = `session/update` `tool_call` carrying `diff` content
//! (no document-change stream exists in v1); "read-mention" = @-mentions
//! arrive as `resource_link` prompt blocks, resolved by the agent via the
//! `fs/read_text_file` request BACK to the IDE; "session-status
//! notifications" = the `session/update` notification family plus the
//! `session/prompt` response `stopReason` (no `session/status` method
//! exists in v1). v2 (draft) is deliberately not implemented.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};

use super::channel::ChannelImpl;
use super::types::{
    AcpContentBlock, PermissionOption, PermissionOutcome, PROTOCOL_VERSION, SessionUpdate,
    StopReason,
};

/// Why the agent core failed one message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcpError {
    /// The wire failed to accept an outbound line.
    #[error("acp transport: {0}")]
    Transport(String),
    /// The engine failed mid-prompt.
    #[error("acp engine: {0}")]
    Engine(String),
    /// An fs/read_text_file request the IDE never answered in time.
    #[error("acp read_text_file timed out for {0}")]
    ReadTimeout(String),
    /// A session/request_permission request the IDE never answered
    /// in time.
    #[error("acp request_permission timed out")]
    PermissionTimeout,
}

/// Outbound line sink (daemon adapts stdio; tests adapt mpsc).
pub trait AcpWire: Send + Sync {
    /// Sends one NDJSON line to the IDE.
    fn send(&self, line: &str) -> Result<(), AcpError>;
}

/// The IDE-facing channel one prompt run sees.
#[async_trait]
pub trait IdeChannel: Send + Sync {
    /// Emits one `session/update` notification.
    fn send_update(&self, update: SessionUpdate);
    /// Reads a mentioned file THROUGH the IDE (`fs/read_text_file`).
    async fn read_text_file(&self, path: &str) -> Result<String, AcpError>;
    /// Whether `session/cancel` arrived for this session.
    fn is_cancelled(&self) -> bool;
    /// The session this channel serves (the engine routes daemon
    /// work by it).
    fn session_id(&self) -> &str;
    /// Asks the IDE to resolve a permission (`session/request_permission`),
    /// bounded by `timeout` (a leak bound only — the caller's own
    /// deadline governs fail-closed resolution).
    async fn request_permission(
        &self,
        options: Vec<PermissionOption>,
        timeout: std::time::Duration,
    ) -> Result<PermissionOutcome, AcpError>;
}

/// The engine behind the ACP surface (the daemon wires the real loop).
#[async_trait]
pub trait AcpEngine: Send + Sync {
    /// Handles one prompt; updates stream through `ide`; the returned
    /// stop reason lands on the `session/prompt` response.
    async fn on_prompt(
        &self,
        prompt: AcpPrompt,
        ide: &dyn IdeChannel,
    ) -> Result<StopReason, AcpError>;
}

/// One prompt: its content blocks (re-exported at the module root).
#[derive(Debug, Clone, PartialEq)]
pub struct AcpPrompt {
    /// The blocks the IDE sent.
    pub blocks: Vec<AcpContentBlock>,
}

pub(super) struct AgentState {
    pub(super) initialized: AtomicBool,
    pub(super) cancels: Mutex<HashSet<String>>,
    pub(super) pending_reads: Mutex<HashMap<i64, tokio::sync::oneshot::Sender<String>>>,
    pub(super) pending_permissions:
        Mutex<HashMap<i64, tokio::sync::oneshot::Sender<PermissionOutcome>>>,
    pub(super) next_request_id: AtomicI64,
    pub(super) next_session: AtomicU64,
}

/// The ACP agent dispatcher.
pub struct AcpAgent {
    engine: Arc<dyn AcpEngine>,
    state: Arc<AgentState>,
}

impl AcpAgent {
    /// Assembles the agent over one engine.
    #[must_use]
    pub fn new(engine: Arc<dyn AcpEngine>) -> Self {
        Self {
            engine,
            state: Arc::new(AgentState {
                initialized: AtomicBool::new(false),
                cancels: Mutex::new(HashSet::new()),
                pending_reads: Mutex::new(HashMap::new()),
                pending_permissions: Mutex::new(HashMap::new()),
                next_request_id: AtomicI64::new(1),
                next_session: AtomicU64::new(0),
            }),
        }
    }

    /// Marks a session cancelled (also driven by the wire notification).
    pub fn mark_cancel(&self, session_id: &str) {
        if let Ok(mut cancels) = self.state.cancels.lock() {
            cancels.insert(session_id.to_owned());
        }
    }

    /// Handles one inbound NDJSON line. Every outbound message
    /// (responses, notifications, agent→IDE requests) flows through
    /// `wire`; long prompt runs are spawned so `session/cancel` can be
    /// processed while they execute.
    pub async fn handle_line(&self, line: &str, wire: &Arc<dyn AcpWire>) -> Result<(), AcpError> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return wire.send(&error_response(
                    Value::Null,
                    -32700,
                    &format!("parse error: {error}"),
                ));
            }
        };
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = message.get("id").cloned();
        match method {
            "initialize" => self.on_initialize(id, &message, wire).await,
            "session/new" => {
                let session = format!(
                    "acp-{}",
                    self.state.next_session.fetch_add(1, Ordering::SeqCst)
                );
                wire.send(&ok_response(
                    id.unwrap_or(Value::Null),
                    json!({ "sessionId": session, "modes": null, "configOptions": null }),
                ))
            }
            "session/prompt" => self.on_prompt(id, &message, wire).await,
            "session/cancel" => {
                if let Some(session) = message
                    .get("params")
                    .and_then(|params| params.get("sessionId"))
                    .and_then(Value::as_str)
                {
                    self.mark_cancel(session);
                }
                Ok(())
            }
            "" => self.on_response(&message),
            other => {
                if id.is_some() {
                    wire.send(&error_response(
                        id.unwrap_or(Value::Null),
                        -32601,
                        &format!("method not found: {other}"),
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    async fn on_initialize(
        &self,
        id: Option<Value>,
        message: &Value,
        wire: &Arc<dyn AcpWire>,
    ) -> Result<(), AcpError> {
        let requested = message
            .get("params")
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_u64);
        let Some(id) = id else { return Ok(()) };
        if requested != Some(PROTOCOL_VERSION) {
            return wire.send(&error_response(
                id,
                -32602,
                &format!("unsupported protocolVersion {requested:?}; this agent speaks {PROTOCOL_VERSION}"),
            ));
        }
        self.state.initialized.store(true, Ordering::SeqCst);
        wire.send(&ok_response(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "agentCapabilities": {
                    "loadSession": false,
                    "promptCapabilities": { "image": false, "audio": false, "embeddedContext": false },
                    "mcpCapabilities": { "http": false, "sse": false }
                },
                "authMethods": [],
                "agentInfo": { "name": "harness", "title": null, "version": env!("CARGO_PKG_VERSION") }
            }),
        ))
    }

    async fn on_prompt(
        &self,
        id: Option<Value>,
        message: &Value,
        wire: &Arc<dyn AcpWire>,
    ) -> Result<(), AcpError> {
        let Some(id) = id else { return Ok(()) };
        let session_id = message
            .get("params")
            .and_then(|params| params.get("sessionId"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let blocks = message
            .get("params")
            .and_then(|params| params.get("prompt"))
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| {
                        serde_json::from_value::<AcpContentBlock>(block.clone()).ok()
                    })
                    .collect::<Vec<AcpContentBlock>>()
            })
            .unwrap_or_default();
        let prompt = AcpPrompt { blocks };
        if let Ok(mut cancels) = self.state.cancels.lock() {
            cancels.remove(&session_id);
        }
        let engine = Arc::clone(&self.engine);
        let state = Arc::clone(&self.state);
        let wire = Arc::clone(wire);
        let session_for_channel = session_id.clone();
        tokio::spawn(async move {
            let channel = ChannelImpl {
                session_id: session_for_channel.clone(),
                state: Arc::clone(&state),
                wire: Arc::clone(&wire),
            };
            let outcome = engine.on_prompt(prompt, &channel).await;
            let payload = match outcome {
                Ok(reason) => ok_response(id, json!({ "stopReason": reason })),
                Err(error) => error_response(id, -32603, &error.to_string()),
            };
            if let Err(error) = wire.send(&payload) {
                tracing::warn!(target: "harness_mcp::acp", %error, "prompt response dropped");
            }
        });
        Ok(())
    }

    fn on_response(&self, message: &Value) -> Result<(), AcpError> {
        let id = message.get("id").and_then(Value::as_i64);
        let Some(id) = id else { return Ok(()) };
        // Permission responses carry the outcome union; read
        // responses carry a plain content string. Route by shape.
        if let Some(outcome) = message
            .get("result")
            .cloned()
            .and_then(|result| serde_json::from_value::<PermissionOutcomeWrapper>(result).ok())
            .map(|wrapper| wrapper.outcome)
        {
            if let Ok(mut pending) = self.state.pending_permissions.lock()
                && let Some(sender) = pending.remove(&id)
            {
                sender.send(outcome).ok();
            }
            return Ok(());
        }
        let content = message
            .get("result")
            .and_then(|result| result.get("content"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if let Ok(mut pending) = self.state.pending_reads.lock()
            && let Some(sender) = pending.remove(&id)
        {
            sender.send(content).ok();
        }
        Ok(())
    }
}

/// Decode helper: the permission response's `result` is the outcome
/// union under a single `outcome` key.
#[derive(serde::Deserialize)]
struct PermissionOutcomeWrapper {
    outcome: PermissionOutcome,
}

fn ok_response(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}
