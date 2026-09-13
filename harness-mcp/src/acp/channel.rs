//! The IDE-facing channel one ACP prompt run sees: `session/update`
//! notifications out, `fs/read_text_file` requests back to the IDE
//! (bounded), and cooperative cancellation.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use super::agent::{AcpError, AcpWire, AgentState, IdeChannel};
use super::types::SessionUpdate;

const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct ChannelImpl {
    pub(super) session_id: String,
    pub(super) state: Arc<AgentState>,
    pub(super) wire: Arc<dyn AcpWire>,
}

impl ChannelImpl {
    async fn read_text_file_inner(&self, path: &str) -> Result<String, AcpError> {
        let request_id = self
            .state
            .next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let Ok(mut pending) = self.state.pending_reads.lock() else {
                return Err(AcpError::Transport("read registry poisoned".to_owned()));
            };
            pending.insert(request_id, tx);
        }
        let request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "fs/read_text_file",
            "params": { "sessionId": self.session_id, "path": path }
        });
        self.wire.send(&request.to_string())?;
        match tokio::time::timeout(READ_TIMEOUT, rx).await {
            Ok(Ok(content)) => Ok(content),
            Ok(Err(_)) => Err(AcpError::Transport("read channel closed".to_owned())),
            Err(_) => {
                if let Ok(mut pending) = self.state.pending_reads.lock() {
                    pending.remove(&request_id);
                }
                Err(AcpError::ReadTimeout(path.to_owned()))
            }
        }
    }
}

#[async_trait]
impl IdeChannel for ChannelImpl {
    fn send_update(&self, update: SessionUpdate) {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": { "sessionId": self.session_id, "update": update }
        });
        if let Err(error) = self.wire.send(&payload.to_string()) {
            tracing::warn!(target: "harness_mcp::acp", %error, "session/update dropped");
        }
    }

    async fn read_text_file(&self, path: &str) -> Result<String, AcpError> {
        self.read_text_file_inner(path).await
    }

    fn is_cancelled(&self) -> bool {
        self.state
            .cancels
            .lock()
            .map(|cancels| cancels.contains(&self.session_id))
            .unwrap_or(false)
    }
}
