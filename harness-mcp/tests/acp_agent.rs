//! T29 ACP agent-side core (Agent Client Protocol v1, Zed-authored):
//! NDJSON JSON-RPC 2.0 framing, `initialize` handshake, session lifecycle
//! (`session/new`, `session/prompt`, `session/cancel`), `session/update`
//! notifications (message chunks + edit-request tool_call diffs), and
//! `fs/read_text_file` (the @-mention read path). Driven by an in-test
//! mock IDE client over raw lines.

use std::sync::Arc;

use async_trait::async_trait;
use harness_mcp::acp::types::{AcpContentBlock, PROTOCOL_VERSION, StopReason};
use harness_mcp::acp::{
    AcpAgent, AcpEngine, AcpError, AcpPrompt, AcpWire, IdeChannel, SessionUpdate,
};

struct PanelEngine {
    mention_seen: Arc<std::sync::Mutex<Option<String>>>,
    edits_seen: Arc<std::sync::Mutex<Vec<String>>>,
    respect_cancel: bool,
}

#[async_trait]
impl AcpEngine for PanelEngine {
    async fn on_prompt(
        &self,
        prompt: AcpPrompt,
        ide: &dyn IdeChannel,
    ) -> Result<StopReason, AcpError> {
        for block in &prompt.blocks {
            if let AcpContentBlock::ResourceLink { uri, .. } = block {
                let content = ide.read_text_file(uri).await?;
                if let Ok(mut seen) = self.mention_seen.lock() {
                    *seen = Some(content);
                }
            }
        }
        ide.send_update(SessionUpdate::agent_message_chunk(
            "panel agent at your service",
        ));
        ide.send_update(SessionUpdate::edit_request(
            "scratch.md",
            Some("old"),
            "new",
        ));
        if let Ok(mut edits) = self.edits_seen.lock() {
            edits.push("scratch.md".to_owned());
        }
        if self.respect_cancel {
            while !ide.is_cancelled() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            return Ok(StopReason::Cancelled);
        }
        Ok(StopReason::EndTurn)
    }
}

#[derive(Clone)]
struct MpscWire {
    to_ide: tokio::sync::mpsc::Sender<String>,
}

impl AcpWire for MpscWire {
    fn send(&self, line: &str) -> Result<(), AcpError> {
        self.to_ide
            .try_send(line.to_owned())
            .map_err(|error| AcpError::Transport(format!("wire: {error}")))
    }
}

fn agent_with(engine: PanelEngine) -> AcpAgent {
    AcpAgent::new(Arc::new(engine))
}

#[tokio::test]
async fn initialize_handshake_negotiates_v1() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let agent = agent_with(PanelEngine {
        mention_seen: Arc::default(),
        edits_seen: Arc::default(),
        respect_cancel: false,
    });
    let wire: Arc<dyn AcpWire> = Arc::new(MpscWire { to_ide: tx });
    agent
        .handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"zed","title":null,"version":"1"}}}"#,
            &wire,
        )
        .await
        .expect("initialize");
    let line = rx.recv().await.expect("response");
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("json");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(parsed["result"]["agentCapabilities"]["loadSession"], false);
    assert!(parsed["result"]["agentInfo"]["name"].is_string());
}

#[tokio::test]
async fn unsupported_protocol_version_is_rejected() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let agent = agent_with(PanelEngine {
        mention_seen: Arc::default(),
        edits_seen: Arc::default(),
        respect_cancel: false,
    });
    let wire: Arc<dyn AcpWire> = Arc::new(MpscWire { to_ide: tx });
    agent
        .handle_line(
            r#"{"jsonrpc":"2.0","id":9,"method":"initialize","params":{"protocolVersion":99,"clientCapabilities":{},"clientInfo":null}}"#,
            &wire,
        )
        .await
        .expect("responds");
    let line = rx.recv().await.expect("response");
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("json");
    assert!(parsed.get("error").is_some(), "{parsed}");
}

#[tokio::test]
async fn session_prompt_streams_updates_reads_mentions_and_stops() {
    let (to_ide_tx, mut to_ide_rx) = tokio::sync::mpsc::channel(32);
    let (to_agent_tx, mut to_agent_rx) = tokio::sync::mpsc::channel(16);
    let mention_seen = Arc::default();
    let edits_seen = Arc::default();
    let agent = Arc::new(agent_with(PanelEngine {
        mention_seen: Arc::clone(&mention_seen),
        edits_seen: Arc::clone(&edits_seen),
        respect_cancel: false,
    }));
    let wire: Arc<dyn AcpWire> = Arc::new(MpscWire { to_ide: to_ide_tx });

    let driver = {
        let agent = Arc::clone(&agent);
        let wire = Arc::clone(&wire);
        tokio::spawn(async move {
            agent
                .handle_line(
                    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":null}}"#,
                    &wire,
                )
                .await
                .expect("init");
            agent
                .handle_line(
                    r#"{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"C:/work"}}"#,
                    &wire,
                )
                .await
                .expect("new session");
            agent
                .handle_line(
                    r#"{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"s-1","prompt":[{"type":"text","text":"refactor this"},{"type":"resource_link","name":"notes","uri":"C:/work/notes.txt"}]}}"#,
                    &wire,
                )
                .await
                .expect("prompt accepted");
        })
    };

    let mut notifications = Vec::new();
    let mut stop_reason = None;
    while let Some(line) = to_ide_rx.recv().await {
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        if parsed["method"] == "fs/read_text_file" {
            to_agent_tx
                .send(format!(
                    r#"{{"jsonrpc":"2.0","id":{},"result":{{"content":"mention body"}}}}"#,
                    parsed["id"]
                ))
                .await
                .ok();
            let agent = Arc::clone(&agent);
            let wire = Arc::clone(&wire);
            let response = to_agent_rx.recv().await.expect("read response line");
            agent
                .handle_line(&response, &wire)
                .await
                .expect("read delivered");
            continue;
        }
        if parsed["method"] == "session/update" {
            notifications.push(
                parsed["params"]["update"]["sessionUpdate"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            );
        }
        if let Some(reason) = parsed["result"]["stopReason"].as_str() {
            stop_reason = Some(reason.to_owned());
            break;
        }
    }
    driver.await.expect("driver");
    assert_eq!(stop_reason.as_deref(), Some("end_turn"));
    assert!(
        notifications.contains(&"agent_message_chunk".to_owned()),
        "{notifications:?}"
    );
    assert!(
        notifications.contains(&"tool_call".to_owned()),
        "{notifications:?}"
    );
    assert_eq!(
        mention_seen.lock().ok().and_then(|guard| guard.clone()),
        Some("mention body".to_owned())
    );
    assert_eq!(edits_seen.lock().ok().map(|guard| guard.len()), Some(1));
}

#[tokio::test]
async fn session_cancel_stops_the_prompt_with_cancelled_reason() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let agent = Arc::new(agent_with(PanelEngine {
        mention_seen: Arc::default(),
        edits_seen: Arc::default(),
        respect_cancel: true,
    }));
    let wire: Arc<dyn AcpWire> = Arc::new(MpscWire { to_ide: tx });
    agent
        .handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":null}}"#,
            &wire,
        )
        .await
        .expect("init");
    agent
        .handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"C:/w"}}"#,
            &wire,
        )
        .await
        .expect("new");

    let prompting = {
        let agent = Arc::clone(&agent);
        let wire = Arc::clone(&wire);
        tokio::spawn(async move {
            agent
                .handle_line(
                    r#"{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"s-1","prompt":[{"type":"text","text":"go"}]}}"#,
                    &wire,
                )
                .await
                .expect("prompt")
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    agent
        .handle_line(
            r#"{"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":"s-1"}}"#,
            &wire,
        )
        .await
        .expect("cancel");

    let mut stop_reason = None;
    while let Some(line) = rx.recv().await {
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        if let Some(reason) = parsed["result"]["stopReason"].as_str() {
            stop_reason = Some(reason.to_owned());
            break;
        }
    }
    prompting.await.expect("prompt task");
    assert_eq!(stop_reason.as_deref(), Some("cancelled"));
}

#[tokio::test]
async fn malformed_lines_and_unknown_methods_are_typed_errors() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let agent = agent_with(PanelEngine {
        mention_seen: Arc::default(),
        edits_seen: Arc::default(),
        respect_cancel: false,
    });
    let wire: Arc<dyn AcpWire> = Arc::new(MpscWire { to_ide: tx });
    agent
        .handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":null}}"#,
            &wire,
        )
        .await
        .expect("init");
    agent
        .handle_line("this is not json at all", &wire)
        .await
        .expect("typed");
    let first = loop {
        let line = rx.recv().await.expect("response");
        if line.contains("error") {
            break line;
        }
    };
    assert!(first.contains("parse error"), "{first}");
    agent
        .handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"session/nope","params":{}}"#,
            &wire,
        )
        .await
        .expect("typed");
    let second = loop {
        let line = rx.recv().await.expect("response");
        if serde_json::from_str::<serde_json::Value>(&line)
            .ok()
            .and_then(|parsed| parsed.get("id").and_then(|id| id.as_i64()))
            == Some(2)
        {
            break line;
        }
    };
    assert!(second.contains("method not found"), "{second}");
}
