//! The one client-protocol pump every transport drives: WebSocket,
//! SSE, and local-socket IPC all convert their frames into the two
//! channels this pump owns (`inbound` client messages, `outbound`
//! server messages). Replay/resync policy, permission routing, idle
//! timeouts, and slow-client eviction live here exactly once.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};

use crate::attach::{AttachError, HostCommand, SessionHost};
use crate::compat::downgrade_for_v1;
use crate::events::ReplayKind;
use crate::protocol::{
    ClientMsg, ErrorCode, PROTOCOL_VERSION, ServerMsg, SessionCommand, version_compatible,
};

/// Why a client session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PumpExit {
    /// Client channel closed (disconnect).
    ClientGone,
    /// Idle read timeout fired (the session survives).
    IdleTimeout,
    /// The hub or control bus closed.
    StreamsClosed,
}

/// Forward one outbound message for a connection, applying the v1
/// wire downgrade when the peer negotiated protocol v1 (media blocks
/// become text markers; v2-only variants are withheld). Returns false
/// when the client is gone.
async fn forward(outbound: &mpsc::Sender<ServerMsg>, peer_version: u32, msg: ServerMsg) -> bool {
    let msg = if peer_version < PROTOCOL_VERSION {
        match downgrade_for_v1(msg) {
            Some(downgraded) => downgraded,
            None => return true,
        }
    } else {
        msg
    };
    outbound.send(msg).await.is_ok()
}

/// Per-connection pump parameters: the negotiated wire version (v1
/// connections receive the downgrade) plus attach and idle policy.
pub struct PumpParams {
    /// The peer's negotiated protocol version.
    pub peer_version: u32,
    /// Last event id the client has seen (replay position).
    pub after: Option<u64>,
    /// Whether the idle read timeout is enforced on this connection.
    pub enforce_idle: bool,
    /// Idle read timeout before the client is dropped.
    pub idle_timeout: Duration,
}

/// Drive one attached client. Sends the replay plan (delta, or resync +
/// snapshot), then fans out live events, control messages, and command
/// acknowledgements until the client goes away or idles out.
///
/// # Errors
/// [`AttachError::TooManyClients`] when the session cap is reached.
pub async fn pump_client(
    host: &Arc<SessionHost>,
    client_id: &str,
    params: PumpParams,
    mut inbound: mpsc::Receiver<ClientMsg>,
    outbound: mpsc::Sender<ServerMsg>,
) -> Result<PumpExit, AttachError> {
    let PumpParams {
        peer_version,
        after,
        enforce_idle,
        idle_timeout,
    } = params;
    let attachment = host.attach(client_id, after)?;
    match attachment.replay.clone() {
        ReplayKind::Delta { frames } => {
            for frame in frames {
                if !forward(&outbound, peer_version, ServerMsg::Event { frame }).await {
                    host.detach(client_id);
                    return Ok(PumpExit::ClientGone);
                }
            }
        }
        ReplayKind::Resync { last_event_id } => {
            let _ = outbound
                .send(ServerMsg::StateResyncRequired { last_event_id })
                .await;
            let snapshot = ServerMsg::Snapshot {
                frames: host.snapshot(),
            };
            if !forward(&outbound, peer_version, snapshot).await {
                host.detach(client_id);
                return Ok(PumpExit::ClientGone);
            }
        }
    }
    let _ = outbound
        .send(ServerMsg::HelloAck {
            version: peer_version,
            last_event_id: attachment.last_event_id,
        })
        .await;

    let mut events = attachment.events.live;
    let mut control = attachment.control;
    let mut last_inbound = tokio::time::Instant::now();
    let mediator = host.mediator();
    let exit = loop {
        let mut idle_deadline = idle_timeout;
        if !enforce_idle {
            idle_deadline = Duration::MAX;
        }
        let since_inbound = last_inbound.elapsed();
        let remaining = idle_deadline
            .checked_sub(since_inbound)
            .unwrap_or(Duration::ZERO);
        tokio::select! {
            biased;
            msg = events.recv() => match msg {
                Ok(server_msg) => {
                    if !forward(&outbound, peer_version, server_msg).await {
                        break PumpExit::ClientGone;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Slow-client eviction: behind the ring head —
                    // forced resync, session preserved.
                    let _ = outbound
                        .send(ServerMsg::StateResyncRequired {
                            last_event_id: host.last_event_id(),
                        })
                        .await;
                    let snapshot = ServerMsg::Snapshot {
                        frames: host.snapshot(),
                    };
                    if !forward(&outbound, peer_version, snapshot).await {
                        break PumpExit::ClientGone;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break PumpExit::StreamsClosed,
            },
            msg = control.recv() => {
                if let Ok(server_msg) = msg
                    && !forward(&outbound, peer_version, server_msg).await
                {
                    break PumpExit::ClientGone;
                }
            }
            msg = inbound.recv() => {
                last_inbound = tokio::time::Instant::now();
                match msg {
                    None => break PumpExit::ClientGone,
                    Some(ClientMsg::Hello { version, .. }) => {
                        if !version_compatible(version) {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::ProtocolVersion,
                                    message: format!("server speaks {PROTOCOL_VERSION}"),
                                })
                                .await;
                            break PumpExit::ClientGone;
                        }
                    }
                    Some(ClientMsg::Ping) => {
                        let _ = outbound.send(ServerMsg::Pong).await;
                    }
                    Some(ClientMsg::Command { command }) => {
                        let host_command = match command {
                            SessionCommand::Submit { text } => HostCommand::Submit(text),
                            SessionCommand::SubmitMedia { text, media } => {
                                HostCommand::SubmitMedia { text, media }
                            }
                            SessionCommand::Steer { text } => HostCommand::Steer(text),
                            SessionCommand::Cancel => HostCommand::Cancel,
                        };
                        if let Err(rejection) = host.handle_command(host_command).await {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::MalformedFrame,
                                    message: rejection.to_string(),
                                })
                                .await;
                        }
                    }
                    Some(ClientMsg::PermissionAnswer { ask_id, answer }) => {
                        if let Err(e) = mediator.answer(ask_id, client_id, answer) {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::Internal,
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    }
                    Some(ClientMsg::PermissionCancel { ask_id }) => {
                        if let Err(e) = mediator.cancel_vote(ask_id, client_id) {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::Internal,
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    }
                    // Fallback answers key on THIS pump's per-connection
                    // id (the 'sse-readonly' discipline) — never a
                    // client-asserted identity.
                    Some(ClientMsg::FallbackAnswer { ask, chosen }) => {
                        if let Err(e) = mediator.answer_fallback(ask, client_id, chosen) {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::Internal,
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    }
                    // Explicit-only rewind: the one path that restores
                    // snapshots (never automatic). The System "restored
                    // to <label>" event rides the hub, so every client
                    // sees it; only the refusal is per-connection.
                    Some(ClientMsg::Rewind { steps }) => {
                        if let Err(e) = crate::rewind::run_rewind(Arc::clone(host), steps).await {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: e.error_code(),
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    }
                    // Plan mode (todo 16): transitions publish on the
                    // hub so every client sees them; there is no
                    // per-connection refusal.
                    Some(ClientMsg::PlanMode { on }) => {
                        crate::plan::set_plan_mode(host, on);
                    }
                    // Plan answers key on THIS pump's per-connection
                    // id exactly like fallback answers — the mediation
                    // policy contract (never a client-asserted
                    // identity).
                    Some(ClientMsg::PlanAnswer { ask, decision }) => {
                        if let Err(e) = mediator.answer_plan(ask, client_id, decision) {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::Internal,
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    }
                }
            }
            _ = tokio::time::sleep(remaining) => {
                let _ = outbound
                    .send(ServerMsg::Bye {
                        reason: "idle read timeout".into(),
                    })
                    .await;
                break PumpExit::IdleTimeout;
            }
        }
    };
    host.detach(client_id);
    Ok(exit)
}
