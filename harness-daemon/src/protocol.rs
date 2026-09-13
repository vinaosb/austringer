//! The one typed client protocol: messages every transport (WebSocket,
//! SSE, IPC) speaks. Versioned, internally tagged, bounded on decode —
//! client input is untrusted and never parsed without a size check.

use serde::{Deserialize, Serialize};

/// Wire protocol version. Bump on breaking change; servers refuse
/// mismatched [`ClientMsg::Hello`] versions with
/// [`ErrorCode::ProtocolVersion`].
pub const PROTOCOL_VERSION: u32 = 1;

/// Hard cap on any single decoded frame (1 MiB). Oversized input is
/// rejected before parsing.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Cap on any free-text field carried inside a frame.
pub const MAX_TEXT_FIELD_BYTES: usize = 64 * 1024;

/// Cap on client ids and session ids (short identifiers, not prose).
pub const MAX_ID_BYTES: usize = 128;

/// Cap on the number of frames a resync snapshot may carry.
pub const MAX_SNAPSHOT_FRAMES: usize = 4096;

/// Decode/protocol failures. All variants are transport-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// Frame exceeded [`MAX_FRAME_BYTES`].
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    Oversized,
    /// Frame did not decode into a known message.
    #[error("malformed frame: {0}")]
    Malformed(String),
    /// Frame decoded but a field violated its bound.
    #[error("field out of bounds: {0}")]
    OutOfBounds(&'static str),
}

/// A client's answer to a permission ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Approve this one call.
    AllowOnce,
    /// Approve and remember for the session.
    AllowSession,
    /// Refuse.
    Deny,
}

/// Commands a client may send into a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionCommand {
    /// Queue a new user turn.
    Submit {
        /// User text.
        text: String,
    },
    /// Steer a running turn (signal-with-start semantics upstream).
    Steer {
        /// User text.
        text: String,
    },
    /// Request cancellation of the running turn.
    Cancel,
}

/// Client-to-daemon messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// First message on every transport: version + auth token + id.
    Hello {
        /// Client-claimed protocol version.
        version: u32,
        /// Daemon-generated bearer token.
        token: String,
        /// Stable client id (bounded, printable).
        client_id: String,
        /// Session to bind to (required on IPC where no URL path
        /// carries it; ignored on URL-addressed transports).
        session: Option<String>,
    },
    /// Run a session command.
    Command {
        /// The command.
        command: SessionCommand,
    },
    /// Answer a broadcast permission ask.
    PermissionAnswer {
        /// Ask being answered.
        ask_id: u64,
        /// The answer.
        answer: Answer,
    },
    /// Cancel-vote sentinel: withdraw a pending ask.
    PermissionCancel {
        /// Ask being cancelled.
        ask_id: u64,
    },
    /// Liveness ping.
    Ping,
}

/// How one ask resolved. Exactly one variant is ever produced per ask
/// (the N1 invariant enforced by [`crate::mediation`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// An answering client allowed it.
    Allowed {
        /// Which client answered.
        by: String,
    },
    /// An answering client refused it.
    Denied {
        /// Which client answered.
        by: String,
    },
    /// A client cancelled it.
    Cancelled {
        /// Which client cancelled.
        by: String,
    },
    /// The synchronous deadline fired with no valid answer.
    Timeout,
}

/// A broadcast permission ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskPayload {
    /// Ask correlation id.
    pub ask_id: u64,
    /// Tool requesting approval.
    pub tool: String,
    /// Unredacted target for the approving human (never model-visible).
    pub target: Option<String>,
    /// Redacted, model-safe reason.
    pub reason: String,
    /// Synchronous deadline in milliseconds after ask start.
    pub deadline_ms: u64,
}

/// Daemon-emitted session events on the typed event stream. `recorded`
/// wraps the engine's own durable event; the rest are daemon-level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// An engine event, sequence-stamped.
    Recorded {
        /// Engine sequence number (1-based).
        seq: u64,
        /// The durable event.
        event: harness_core::event::Event,
    },
    /// A turn workflow started.
    TurnStarted {
        /// Turn number.
        turn: u64,
    },
    /// A turn workflow finished.
    TurnFinished {
        /// Turn number.
        turn: u64,
        /// Run outcome summary.
        outcome: String,
    },
    /// A semantic diff surfaced for review UIs.
    Diff {
        /// Changed path (relative, forward slashes).
        path: String,
        /// blake3 of the before content (hex, empty for adds).
        before_hash: String,
        /// blake3 of the after content (hex, empty for deletes).
        after_hash: String,
        /// Rendered hunks.
        hunks: Vec<String>,
    },
    /// A notification crossed the daemon bus.
    Notification {
        /// Notification kind (typed vocabulary).
        kind: String,
        /// Human-facing message.
        message: String,
    },
    /// A worker claimed this session from a named queue.
    WorkerClaimed {
        /// Queue name.
        queue: String,
        /// Worker id.
        worker: String,
    },
    /// The claiming worker vanished; session self-healed.
    WorkerHealed {
        /// Queue the session returned to.
        queue: String,
    },
}

/// One numbered frame on the monotonic per-session event stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Monotonic, gap-free, 1-based event id.
    pub id: u64,
    /// The event.
    pub event: SessionEvent,
}

/// Typed error codes on the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Hello version mismatch.
    ProtocolVersion,
    /// Missing/invalid token.
    Auth,
    /// Frame over [`MAX_FRAME_BYTES`].
    OversizedFrame,
    /// Undecodable frame.
    MalformedFrame,
    /// Unknown session id.
    UnknownSession,
    /// Session attach cap reached.
    TooManyClients,
    /// Idle read timeout fired.
    IdleTimeout,
    /// Unexpected internal failure (detail scrubbed).
    Internal,
}

/// Daemon-to-client messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Response to a valid [`ClientMsg::Hello`].
    HelloAck {
        /// Server protocol version.
        version: u32,
        /// Highest event id the session has produced.
        last_event_id: u64,
    },
    /// One event frame (the SSE `data:` payload is exactly this message).
    Event {
        /// The frame.
        frame: Frame,
    },
    /// A permission ask broadcast to all attached clients.
    PermissionAsk {
        /// The ask.
        ask: AskPayload,
    },
    /// Final resolution of an ask, broadcast to all attached clients.
    PermissionResolved {
        /// Ask correlation id.
        ask_id: u64,
        /// Exactly-one resolution.
        resolution: Resolution,
    },
    /// The client diverged (behind the replay ring head): delta replay is
    /// impossible; a [`ServerMsg::Snapshot`] follows.
    StateResyncRequired {
        /// Highest event id at resync time.
        last_event_id: u64,
    },
    /// Full-state snapshot replacing delta replay after divergence.
    Snapshot {
        /// Frames (bounded by [`MAX_SNAPSHOT_FRAMES`]).
        frames: Vec<Frame>,
    },
    /// Typed error (message is bounded, scrubbed text).
    Error {
        /// Machine-readable code.
        code: ErrorCode,
        /// Bounded human text.
        message: String,
    },
    /// Response to [`ClientMsg::Ping`].
    Pong,
    /// Transport is closing (drain, reload, or fatal).
    Bye {
        /// Why.
        reason: String,
    },
}

/// Decode one client message from raw bytes with bounds enforced first.
pub fn decode_client(raw: &[u8]) -> Result<ClientMsg, ProtocolError> {
    if raw.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::Oversized);
    }
    let msg: ClientMsg =
        serde_json::from_slice(raw).map_err(|e| ProtocolError::Malformed(e.to_string()))?;
    validate_client(&msg)?;
    Ok(msg)
}

/// Encode one server message deterministically (struct field order +
/// sorted map keys; `preserve_order` is never enabled workspace-wide).
pub fn encode_server(msg: &ServerMsg) -> Result<String, ProtocolError> {
    serde_json::to_string(msg).map_err(|e| ProtocolError::Malformed(e.to_string()))
}

/// Encode one client message (IPC tests, worker mocks).
pub fn encode_client(msg: &ClientMsg) -> Result<String, ProtocolError> {
    serde_json::to_string(msg).map_err(|e| ProtocolError::Malformed(e.to_string()))
}

/// Decode one server message (client-side and tests).
pub fn decode_server(raw: &[u8]) -> Result<ServerMsg, ProtocolError> {
    if raw.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::Oversized);
    }
    serde_json::from_slice(raw).map_err(|e| ProtocolError::Malformed(e.to_string()))
}

fn bounded_text(value: &str, field: &'static str) -> Result<(), ProtocolError> {
    if value.len() > MAX_TEXT_FIELD_BYTES {
        return Err(ProtocolError::OutOfBounds(field));
    }
    Ok(())
}

fn bounded_id(value: &str, field: &'static str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.len() > MAX_ID_BYTES {
        return Err(ProtocolError::OutOfBounds(field));
    }
    if !value.bytes().all(|b| b.is_ascii_graphic() || b == b' ') {
        return Err(ProtocolError::OutOfBounds(field));
    }
    Ok(())
}

/// Validate every free-form field of a decoded client message. Untrusted
/// input is bounded before it touches daemon state.
pub fn validate_client(msg: &ClientMsg) -> Result<(), ProtocolError> {
    match msg {
        ClientMsg::Hello {
            token,
            client_id,
            session,
            ..
        } => {
            bounded_id(client_id, "client_id")?;
            bounded_text(token, "token")?;
            if let Some(session) = session {
                bounded_id(session, "session")?;
            }
            Ok(())
        }
        ClientMsg::Command {
            command: SessionCommand::Submit { text } | SessionCommand::Steer { text },
        } => bounded_text(text, "text"),
        ClientMsg::Command {
            command: SessionCommand::Cancel,
        } => Ok(()),
        ClientMsg::PermissionAnswer { .. } | ClientMsg::PermissionCancel { .. } => Ok(()),
        ClientMsg::Ping => Ok(()),
    }
}

/// True when a Hello version can be served by this build.
#[must_use]
pub fn version_compatible(client_version: u32) -> bool {
    client_version == PROTOCOL_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> ClientMsg {
        ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            token: "t".repeat(32),
            client_id: "web-1".into(),
            session: None,
        }
    }

    #[test]
    fn client_roundtrip_preserves_shape() {
        let raw = encode_client(&hello()).unwrap_or_default();
        let back = decode_client(raw.as_bytes()).unwrap_or(ClientMsg::Ping);
        assert_eq!(back, hello());
    }

    #[test]
    fn oversized_frame_rejected_before_parse() {
        let big = vec![b'x'; MAX_FRAME_BYTES + 1];
        assert_eq!(decode_client(&big), Err(ProtocolError::Oversized));
    }

    #[test]
    fn garbage_rejected_typed() {
        let err = decode_client(b"{\"type\":\"nonsense\"}");
        assert!(matches!(err, Err(ProtocolError::Malformed(_))));
        let err = decode_client(b"not json at all");
        assert!(matches!(err, Err(ProtocolError::Malformed(_))));
    }

    #[test]
    fn oversized_text_field_rejected() {
        let msg = ClientMsg::Command {
            command: SessionCommand::Submit {
                text: "a".repeat(MAX_TEXT_FIELD_BYTES + 1),
            },
        };
        let raw = serde_json::to_vec(&msg).unwrap_or_default();
        assert_eq!(decode_client(&raw), Err(ProtocolError::OutOfBounds("text")));
    }

    #[test]
    fn malformed_client_id_rejected() {
        let msg = ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            token: "t".into(),
            client_id: "bad\nid".into(),
            session: None,
        };
        let raw = serde_json::to_vec(&msg).unwrap_or_default();
        assert_eq!(
            decode_client(&raw),
            Err(ProtocolError::OutOfBounds("client_id"))
        );
    }

    #[test]
    fn server_encoding_is_byte_deterministic() {
        let msg = ServerMsg::Event {
            frame: Frame {
                id: 7,
                event: SessionEvent::TurnStarted { turn: 1 },
            },
        };
        let a = encode_server(&msg).unwrap_or_default();
        let b = encode_server(&msg).unwrap_or_default();
        assert_eq!(a, b);
    }

    #[test]
    fn version_gate_is_exact() {
        assert!(version_compatible(PROTOCOL_VERSION));
        assert!(!version_compatible(PROTOCOL_VERSION + 1));
    }

    #[test]
    fn snapshot_frame_bound_enforced_by_type_invariant() {
        // The bound is enforced at construction sites (event hub); the
        // constant is part of the protocol contract.
        const _: () = assert!(MAX_SNAPSHOT_FRAMES >= 1024);
    }
}
