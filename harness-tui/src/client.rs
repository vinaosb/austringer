//! Daemon WebSocket client task: the only async piece of the TUI. It
//! owns the socket, the Hello handshake, the `after` replay cursor,
//! and capped reconnection, and bridges the daemon to the synchronous
//! UI loop through two channels. The UI loop never touches sockets or
//! timers; this task sleeps, the loop does not.

use std::sync::mpsc::Sender;

use futures_util::{SinkExt, StreamExt};
use harness_daemon::protocol::{
    ClientMsg, PROTOCOL_VERSION, ServerMsg, decode_server, encode_client,
};
use tokio::sync::mpsc::Receiver;

/// Client-task failures, typed for the status area.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    /// Could not reach the endpoint (daemon absent).
    #[error("daemon unreachable: {0}")]
    Unreachable(String),
    /// The endpoint refused the handshake (auth token, route).
    #[error("handshake refused: {0}")]
    Refused(String),
    /// The server stream broke the protocol contract.
    #[error("protocol violation: {0}")]
    Protocol(String),
}

/// Events the client task reports to the UI loop.
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// Handshake complete (`HelloAck` seen).
    Attached {
        /// Highest event id the daemon reports for the session.
        last_event_id: u64,
    },
    /// One daemon message.
    Server(ServerMsg),
    /// An established connection was lost; the task reconnects (or
    /// parks, see [`ClientEvent::Failed`]).
    Dropped,
    /// The task gave up and parked until a `Reconnect` command.
    Failed(ClientError),
}

/// UI-loop commands to the client task.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientCmd {
    /// Send one protocol message.
    Send(ClientMsg),
    /// Tear down (if any) and reconnect now.
    Reconnect,
    /// Stop the task.
    Shutdown,
}

/// Malformed server frames tolerated before the client disconnects
/// itself (mirrors the daemon's own three-strikes bound).
const MALFORMED_STRIKES: u32 = 3;

/// Failed connect attempts in a row (before any attach) tolerated
/// before parking for manual reconnect.
const CONNECT_ATTEMPTS: u32 = 3;

/// Keep-alive cadence, well under the daemon's 300 s idle timeout.
const PING_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// Backoff between failed attempts (fixed; three attempts max, so a
/// graduated ladder would only add complexity).
const RETRY_PAUSE: std::time::Duration = std::time::Duration::from_millis(50);

/// Build the attach URL: `http://127.0.0.1:PORT` base + session +
/// token + replay cursor.
#[must_use]
pub fn ws_attach_url(base: &str, session: &str, token: &str, after: Option<u64>) -> String {
    let host = base
        .strip_prefix("http://")
        .or_else(|| base.strip_prefix("ws://"))
        .unwrap_or(base);
    match after {
        Some(id) => format!("ws://{host}/s/{session}/attach?token={token}&after={id}"),
        None => format!("ws://{host}/s/{session}/attach?token={token}"),
    }
}

/// Run the client until `Shutdown`. On a lost connection the task
/// reconnects with `after = last seen frame id` (the stale-state
/// replay path); after [`CONNECT_ATTEMPTS`] consecutive failures it
/// reports [`ClientEvent::Failed`] and parks until a `Reconnect`.
pub async fn run(
    base: &str,
    session: &str,
    token: &str,
    client_id: &str,
    events: Sender<ClientEvent>,
    mut cmd: Receiver<ClientCmd>,
) {
    let mut cursor: Option<u64> = None;
    let mut failures: u32 = 0;
    'task: loop {
        let attempt =
            session_once(base, session, token, client_id, cursor, &events, &mut cmd).await;
        match attempt {
            Attempt::Shutdown => break 'task,
            Attempt::Lost { last } => {
                cursor = merge_option(cursor, last);
                let _ = events.send(ClientEvent::Dropped);
                failures = 0;
            }
            Attempt::Failed(err) => {
                // A protocol violation after a good attach is fatal:
                // the server proved reachable and hostile/broken, so
                // reconnecting would just repeat it. Connect failures
                // count toward the capped auto-retry.
                let fatal = matches!(err, ClientError::Protocol(_));
                failures += 1;
                if fatal || failures >= CONNECT_ATTEMPTS {
                    let _ = events.send(ClientEvent::Failed(err));
                    loop {
                        match cmd.recv().await {
                            Some(ClientCmd::Reconnect) => {
                                failures = 0;
                                continue 'task;
                            }
                            Some(ClientCmd::Send(_)) => {}
                            _ => break 'task,
                        }
                    }
                }
                tokio::time::sleep(RETRY_PAUSE).await;
            }
        }
    }
}

enum Attempt {
    /// Attached and ran until the socket broke; `last` is the highest
    /// frame id observed on that connection.
    Lost {
        last: Option<u64>,
    },
    /// Never attached (or the server violated the protocol).
    Failed(ClientError),
    Shutdown,
}

/// One full connection lifecycle: connect, handshake, pump both
/// directions until the socket breaks, a `Bye`, a command, or the
/// malformed-frame bound.
async fn session_once(
    base: &str,
    session: &str,
    token: &str,
    client_id: &str,
    cursor: Option<u64>,
    events: &Sender<ClientEvent>,
    cmd: &mut Receiver<ClientCmd>,
) -> Attempt {
    let url = ws_attach_url(base, session, token, cursor);
    let (mut ws, _response) = match tokio_tungstenite::connect_async(url).await {
        Ok(connected) => connected,
        Err(error) => return Attempt::Failed(classify_connect(&error)),
    };
    let hello = ClientMsg::Hello {
        version: PROTOCOL_VERSION,
        token: token.to_owned(),
        client_id: client_id.to_owned(),
        session: Some(session.to_owned()),
    };
    let Ok(encoded) = encode_client(&hello) else {
        return Attempt::Failed(ClientError::Protocol("hello encode".into()));
    };
    if ws
        .send(tokio_tungstenite::tungstenite::Message::Text(
            encoded.into(),
        ))
        .await
        .is_err()
    {
        return Attempt::Failed(ClientError::Refused("hello rejected".into()));
    }
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.reset();
    let mut malformed = 0u32;
    let mut last: Option<u64> = None;
    loop {
        tokio::select! {
            biased;
            command = cmd.recv() => match command {
                Some(ClientCmd::Send(msg)) => {
                    if let Ok(text) = encode_client(&msg)
                        && ws
                            .send(tokio_tungstenite::tungstenite::Message::Text(text.into()))
                            .await
                            .is_err()
                    {
                        return Attempt::Lost { last };
                    }
                }
                Some(ClientCmd::Reconnect) => return Attempt::Lost { last },
                _ => return Attempt::Shutdown,
            },
            _ = ping.tick() => {
                if let Ok(text) = encode_client(&ClientMsg::Ping)
                    && ws
                        .send(tokio_tungstenite::tungstenite::Message::Text(text.into()))
                        .await
                        .is_err()
                {
                    return Attempt::Lost { last };
                }
            }
            inbound = ws.next() => match inbound {
                Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                    match decode_server(text.as_bytes()) {
                        Ok(msg) => {
                            if matches!(msg, ServerMsg::Bye { .. }) {
                                return Attempt::Lost { last };
                            }
                            last = merge_last(last, &msg);
                            if let ServerMsg::HelloAck { last_event_id, .. } = &msg {
                                let _ = events.send(ClientEvent::Attached {
                                    last_event_id: *last_event_id,
                                });
                            }
                            let _ = events.send(ClientEvent::Server(msg));
                        }
                        Err(_) => {
                            malformed += 1;
                            if malformed >= MALFORMED_STRIKES {
                                return Attempt::Failed(ClientError::Protocol(
                                    "three malformed frames".into(),
                                ));
                            }
                        }
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return Attempt::Lost { last },
            },
        }
    }
}

/// Highest event id hinted by one server message (replay cursor feed).
fn merge_last(current: Option<u64>, msg: &ServerMsg) -> Option<u64> {
    let hinted = match msg {
        ServerMsg::Event { frame } => Some(frame.id),
        ServerMsg::HelloAck { last_event_id, .. } => Some(*last_event_id),
        ServerMsg::StateResyncRequired { last_event_id } => Some(*last_event_id),
        _ => None,
    };
    merge_option(current, hinted)
}

fn merge_option(known: Option<u64>, seen: Option<u64>) -> Option<u64> {
    match (known, seen) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, seen) => seen,
    }
}

fn classify_connect(error: &tokio_tungstenite::tungstenite::Error) -> ClientError {
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            ClientError::Refused(format!("http {}", response.status().as_u16()))
        }
        _ => ClientError::Unreachable(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_url_builds_from_http_base_with_and_without_cursor() {
        assert_eq!(
            ws_attach_url("http://127.0.0.1:9000", "s1", "tok", None),
            "ws://127.0.0.1:9000/s/s1/attach?token=tok"
        );
        assert_eq!(
            ws_attach_url("127.0.0.1:9000", "s1", "tok", Some(41)),
            "ws://127.0.0.1:9000/s/s1/attach?token=tok&after=41"
        );
    }

    #[test]
    fn merge_last_keeps_the_highest_hint() {
        let mut last = None;
        last = merge_last(
            last,
            &ServerMsg::HelloAck {
                version: 1,
                last_event_id: 5,
            },
        );
        last = merge_last(last, &ServerMsg::Pong);
        assert_eq!(last, Some(5));
    }
}
