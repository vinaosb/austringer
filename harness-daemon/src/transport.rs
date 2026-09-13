//! The one client-protocol pump every transport drives: WebSocket,
//! SSE, and local-socket IPC all convert their frames into the two
//! channels this pump owns (`inbound` client messages, `outbound`
//! server messages). Replay/resync policy, permission routing, idle
//! timeouts, and slow-client eviction live here exactly once.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};

use crate::attach::{AttachError, HostCommand, SessionHost};
use crate::events::ReplayKind;
use crate::mediation::MediationError;
use crate::protocol::{ClientMsg, ErrorCode, PROTOCOL_VERSION, ServerMsg, SessionCommand};

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

/// Drive one attached client. Sends the replay plan (delta, or resync +
/// snapshot), then fans out live events, control messages, and command
/// acknowledgements until the client goes away or idles out.
///
/// # Errors
/// [`AttachError::TooManyClients`] when the session cap is reached.
pub async fn pump_client(
    host: &Arc<SessionHost>,
    client_id: &str,
    after: Option<u64>,
    enforce_idle: bool,
    idle_timeout: Duration,
    mut inbound: mpsc::Receiver<ClientMsg>,
    outbound: mpsc::Sender<ServerMsg>,
) -> Result<PumpExit, AttachError> {
    let attachment = host.attach(client_id, after)?;
    match attachment.replay.clone() {
        ReplayKind::Delta { frames } => {
            for frame in frames {
                let _ = outbound.send(ServerMsg::Event { frame }).await;
            }
        }
        ReplayKind::Resync { last_event_id } => {
            let _ = outbound
                .send(ServerMsg::StateResyncRequired { last_event_id })
                .await;
            let _ = outbound
                .send(ServerMsg::Snapshot {
                    frames: host.snapshot(),
                })
                .await;
        }
    }
    let _ = outbound
        .send(ServerMsg::HelloAck {
            version: PROTOCOL_VERSION,
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
                    if outbound.send(server_msg).await.is_err() {
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
                    let _ = outbound
                        .send(ServerMsg::Snapshot {
                            frames: host.snapshot(),
                        })
                        .await;
                }
                Err(broadcast::error::RecvError::Closed) => break PumpExit::StreamsClosed,
            },
            msg = control.recv() => {
                if let Ok(server_msg) = msg
                    && outbound.send(server_msg).await.is_err()
                {
                    break PumpExit::ClientGone;
                }
            }
            msg = inbound.recv() => {
                last_inbound = tokio::time::Instant::now();
                match msg {
                    None => break PumpExit::ClientGone,
                    Some(ClientMsg::Hello { version, .. }) => {
                        if version != PROTOCOL_VERSION {
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
                            SessionCommand::Steer { text } => HostCommand::Steer(text),
                            SessionCommand::Cancel => HostCommand::Cancel,
                        };
                        host.handle_command(host_command).await;
                    }
                    Some(ClientMsg::PermissionAnswer { ask_id, answer }) => {
                        if let Err(MediationError::UnknownAsk(ask_id)) =
                            mediator.answer(ask_id, client_id, answer)
                        {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::Internal,
                                    message: format!("unknown ask {ask_id}"),
                                })
                                .await;
                        }
                    }
                    Some(ClientMsg::PermissionCancel { ask_id }) => {
                        if let Err(MediationError::UnknownAsk(ask_id)) =
                            mediator.cancel_vote(ask_id, client_id)
                        {
                            let _ = outbound
                                .send(ServerMsg::Error {
                                    code: ErrorCode::Internal,
                                    message: format!("unknown ask {ask_id}"),
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
