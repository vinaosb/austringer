//! Todo 12 (ecosystem alignment): `harness-daemon --acp` — the ACP
//! stdio agent mode. Exposes harness-mcp's AcpAgent as a process an
//! IDE host (Zed, JetBrains, Cursor) spawns; each ACP session maps
//! 1:1 onto a daemon session in the SAME store root
//! (`{data-dir}/sessions/{acp-session-id}`). MODE DECISION: stdio is
//! the ONLY surface in `--acp` (no HTTP/WS/IPC listener, no
//! scheduler) — the process lives exactly as long as the IDE pipe.
//!
//! Mediation contract: the bridge joins each session's permission
//! queue under the SERVER-ASSIGNED id [`ACP_CLIENT_ID`] (never
//! client-assertable), subject to the session's `MediationPolicy` —
//! an ACP answer counts exactly where a local client's would, and
//! nowhere more (Designated/LocalOnly policies ignore it; the
//! deadline denies fail-closed). `FallbackAsk` and plan approval are
//! NOT answerable over ACP v1: the bridge observes them and never
//! answers — the mediation deadline routes deny/same-model-retry per
//! the task-9 dispatcher contract. `session/cancel` forwards into the
//! daemon's Cancel path (the same queue every wire client uses; the
//! engine driver defers it to the turn boundary, so the sticky token
//! lands on the next prompt — identical semantics to WS/SSE/IPC).
//!
//! Receipts: Zed 1.21 ACP async wakeup; Cursor CLI ACP child
//! sessions; goose W3C trace ACP→MCP; Gemini CLI usage_update;
//! research §23.8 ADOPT.

use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use harness_core::event::{ContentBlock, Event};
use harness_mcp::acp::types::{
    PermissionOption, PermissionOptionKind, PermissionOutcome, SessionUpdate, StopReason,
    ToolCallContent,
};
use harness_mcp::acp::{AcpEngine, AcpError, AcpPrompt, AcpWire, IdeChannel};
use serde_json::Value;

use crate::attach::{HostCommand, SessionHost};
use crate::protocol::{Answer, ServerMsg, SessionEvent};
use crate::server::DaemonState;

/// The distinct, server-assigned identity the bridge answers
/// permission asks under (the mediation-queue contract).
pub const ACP_CLIENT_ID: &str = "acp-stdio";

/// Slack over the session ask deadline for the ACP permission
/// roundtrip: the mediator deadline resolves the ask fail-closed;
/// this only unblocks the pump loop afterwards.
const PERMISSION_SLACK: Duration = Duration::from_secs(5);

/// How often the pump loop re-checks ACP cancellation while a turn
/// runs quietly (inference in flight, no hub traffic).
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// The stdout NDJSON sink (ACP owns stdout in this mode; stdout().lock()
/// serializes concurrent writers).
struct StdioWire;

impl AcpWire for StdioWire {
    fn send(&self, line: &str) -> Result<(), AcpError> {
        let mut out = std::io::stdout().lock();
        writeln!(out, "{line}")
            .and_then(|()| out.flush())
            .map_err(|error| AcpError::Transport(format!("stdout: {error}")))
    }
}

/// The AcpEngine behind the stdio agent: one prompt = one daemon
/// session turn, hub events mapped onto `session/update`
/// notifications, permission asks mapped onto ACP permission
/// requests answered back through the mediator.
struct Bridge {
    state: Arc<DaemonState>,
}

#[async_trait]
impl AcpEngine for Bridge {
    async fn on_prompt(
        &self,
        prompt: AcpPrompt,
        ide: &dyn IdeChannel,
    ) -> Result<StopReason, AcpError> {
        let host = self.state.session(ide.session_id());
        let text = compose_prompt(&prompt.blocks, ide).await?;
        if text.trim().is_empty() {
            return Err(AcpError::Engine("empty prompt".to_owned()));
        }
        let mut hub = host.hub_subscribe();
        let mut control = host.mediator().subscribe();
        let baseline = host.last_event_id();
        host.handle_command(HostCommand::Submit(text))
            .await
            .map_err(|rejection| AcpError::Engine(rejection.to_string()))?;
        let permission_bound = self.state.config.ask_timeout + PERMISSION_SLACK;
        let mut cancel_sent = false;
        loop {
            if !cancel_sent && ide.is_cancelled() {
                cancel_sent = true;
                let _ = host.handle_command(HostCommand::Cancel).await;
            }
            tokio::select! {
                msg = hub.recv() => match msg {
                    Ok(ServerMsg::Event { frame }) => {
                        if frame.id <= baseline {
                            continue;
                        }
                        if let Some(stop) = forward_event(&frame.event, ide) {
                            return Ok(stop);
                        }
                    }
                    // Behind the ring head: catch up through the
                    // snapshot so the turn's finish is never missed.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        for frame in host.snapshot() {
                            if frame.id <= baseline {
                                continue;
                            }
                            if let Some(stop) = forward_event(&frame.event, ide) {
                                return Ok(stop);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(AcpError::Engine("session hub closed".to_owned()));
                    }
                    // Transient hub broadcasts (Usage, MediaResult, …)
                    // have no ACP v1 mapping; asks ride the control bus.
                    Ok(_) => {}
                },
                msg = control.recv() => {
                    if let Ok(ServerMsg::PermissionAsk { ask }) = msg {
                        answer_through_ide(ide, &host, ask.ask_id, permission_bound).await;
                    }
                    // FallbackAsk / resolved frames are observed,
                    // never answered over ACP v1 (module docs).
                }
                _ = tokio::time::sleep(CANCEL_POLL) => {}
            }
        }
    }
}

/// Flattens prompt blocks into the daemon submit: text verbatim,
/// @-mentions read THROUGH the IDE (`fs/read_text_file`) and inlined
/// as labeled context.
async fn compose_prompt(
    blocks: &[harness_mcp::acp::AcpContentBlock],
    ide: &dyn IdeChannel,
) -> Result<String, AcpError> {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            harness_mcp::acp::AcpContentBlock::Text { text } => parts.push(text.clone()),
            harness_mcp::acp::AcpContentBlock::ResourceLink { name, uri } => {
                let content = ide.read_text_file(uri).await?;
                parts.push(format!("[@{name}] ({uri})\n{content}"));
            }
        }
    }
    Ok(parts.join("\n\n"))
}

/// Maps one session event onto `session/update` notifications;
/// `Some(stop)` when the turn finished.
fn forward_event(event: &SessionEvent, ide: &dyn IdeChannel) -> Option<StopReason> {
    match event {
        SessionEvent::Recorded {
            event: Event::Assistant { blocks },
            ..
        } => {
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    ide.send_update(SessionUpdate::agent_message_chunk(text));
                }
            }
            None
        }
        SessionEvent::Recorded {
            event:
                Event::ToolCall {
                    call_id,
                    tool,
                    arguments,
                    ..
                },
            ..
        } => {
            ide.send_update(tool_call_update(call_id, tool, arguments));
            None
        }
        SessionEvent::TurnFinished { outcome, .. } => Some(match outcome.as_str() {
            "turn_complete" | "task_completed" => StopReason::EndTurn,
            cancelled if cancelled.starts_with("cancelled") => StopReason::Cancelled,
            failed => {
                ide.send_update(SessionUpdate::agent_message_chunk(&format!(
                    "turn failed: {failed}"
                )));
                StopReason::Refusal
            }
        }),
        _ => None,
    }
}

/// One tool call as an ACP update. Edit proposals carry their diff
/// content derived from the recorded arguments (executor-independent:
/// worker-backed and registry-backed execution both surface here).
fn tool_call_update(call_id: &str, tool: &str, arguments: &Value) -> SessionUpdate {
    let (kind, content) = if tool == "edit_lines" {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let diffs = arguments
            .get("edits")
            .and_then(Value::as_array)
            .map(|edits| {
                edits
                    .iter()
                    .map(|edit| ToolCallContent::Diff {
                        path: path.to_owned(),
                        old_text: edit
                            .get("old_text")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        new_text: edit
                            .get("new_text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        ("edit", diffs)
    } else {
        (
            match tool {
                "view_window" | "lsp_query" | "tool_search" | "search_sessions" => "read",
                "execute_command" => "execute",
                _ => "other",
            },
            Vec::new(),
        )
    };
    SessionUpdate::ToolCall {
        tool_call_id: call_id.to_owned(),
        title: tool.to_owned(),
        kind: kind.to_owned(),
        content,
    }
}

/// Serves one permission ask: ACP request → IDE answer → mediator
/// vote under [`ACP_CLIENT_ID`] (fail-closed on every unanswered
/// shape — the mediator deadline denies).
async fn answer_through_ide(
    ide: &dyn IdeChannel,
    host: &Arc<SessionHost>,
    ask_id: u64,
    bound: Duration,
) {
    let options = vec![
        PermissionOption::from_kind(PermissionOptionKind::AllowOnce),
        PermissionOption::from_kind(PermissionOptionKind::AllowAlways),
        PermissionOption::from_kind(PermissionOptionKind::RejectOnce),
    ];
    let outcome = ide.request_permission(options, bound).await;
    let mediator = host.mediator();
    match outcome {
        Ok(PermissionOutcome::Selected { option_id }) => {
            // Unknown ids resolve deny-side: fail-closed by mapping.
            let answer = match option_id.as_str() {
                "allow_once" => Answer::AllowOnce,
                "allow_always" => Answer::AllowSession,
                _ => Answer::Deny,
            };
            if let Err(error) = mediator.answer(ask_id, ACP_CLIENT_ID, answer) {
                tracing::warn!(target: "harness_daemon::acp", %error, "late ACP permission answer dropped");
            }
        }
        Ok(PermissionOutcome::Cancelled) => {
            if let Err(error) = mediator.cancel_vote(ask_id, ACP_CLIENT_ID) {
                tracing::warn!(target: "harness_daemon::acp", %error, "late ACP permission cancel dropped");
            }
        }
        Err(_) => {}
    }
}

/// Runs the ACP stdio agent until stdin closes (the IDE ending the
/// process). Stdout carries ONLY ACP lines in this mode.
///
/// # Errors
/// stdin read failure.
pub async fn run_acp_stdio(state: Arc<DaemonState>) -> Result<(), String> {
    let agent = Arc::new(harness_mcp::acp::AcpAgent::new(Arc::new(Bridge { state })));
    let wire: Arc<dyn AcpWire> = Arc::new(StdioWire);
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut line = String::new();
    loop {
        line.clear();
        let read = tokio::io::AsyncBufReadExt::read_line(&mut stdin, &mut line)
            .await
            .map_err(|error| format!("stdin: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        agent
            .handle_line(trimmed, &wire)
            .await
            .map_err(|error| format!("acp: {error}"))?;
    }
}
