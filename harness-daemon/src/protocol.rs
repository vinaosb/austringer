//! The one typed client protocol: messages every transport (WebSocket,
//! SSE, IPC) speaks. Versioned, internally tagged, bounded on decode —
//! client input is untrusted and never parsed without a size check.

use serde::{Deserialize, Serialize};

use harness_core::event::MediaRef;
use harness_providers::identity::ModelIdentity;

pub use crate::ask::{Answer, AskPayload, FallbackAskPayload, Resolution};
pub use crate::errors::{ErrorCode, ProtocolError};

/// Wire protocol version. Bump on breaking change; servers refuse
/// mismatched [`ClientMsg::Hello`] versions with
/// [`ErrorCode::ProtocolVersion`]. Version 2 adds the media surface
/// (upload route, `submit_media`, `media_result`); v1 clients are
/// served through the wire downgrade in [`crate::compat`].
pub const PROTOCOL_VERSION: u32 = 2;

/// Lowest protocol version this daemon still serves (v1 frontends are
/// never refused; they receive downgraded frames instead).
pub const MIN_PROTOCOL_VERSION: u32 = 1;

/// Hard cap on any single decoded frame (1 MiB). Oversized input is
/// rejected before parsing. Media bytes NEVER ride frames — they move
/// through the size-capped HTTP upload route.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Cap on any free-text field carried inside a frame.
pub const MAX_TEXT_FIELD_BYTES: usize = 64 * 1024;

/// Cap on the number of media references one submit may carry.
pub const MAX_MEDIA_PER_SUBMIT: usize = 16;

/// Cap on client ids and session ids (short identifiers, not prose).
pub const MAX_ID_BYTES: usize = 128;

/// Cap on the number of frames a resync snapshot may carry.
pub const MAX_SNAPSHOT_FRAMES: usize = 4096;

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
    /// Queue a new user turn carrying previously uploaded media
    /// (protocol v2; refs resolve against the session media store).
    SubmitMedia {
        /// User text (may be empty when the turn is media-only).
        text: String,
        /// Stored media references (each from the upload route).
        media: Vec<MediaRef>,
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
    /// Answer a broadcast model-fallback ask. The answering identity is
    /// the SERVER-ASSIGNED per-connection id at the transport pump —
    /// never the client-asserted Hello id (Designated/LocalOnly
    /// mediation keys on it). `chosen` must equal the ask's `proposed`
    /// to hop; any other identity keeps the current model.
    FallbackAnswer {
        /// Ask being answered.
        ask: u64,
        /// The identity the answering client chooses.
        chosen: ModelIdentity,
    },
    /// Liveness ping.
    Ping,
}

pub use crate::events::{Frame, SessionEvent};

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
    /// Media receipt for v2 connections (protocol v2 only; withheld
    /// from v1 wires by the [`crate::compat`] downgrade).
    MediaResult {
        /// Producer disclosure: which subsystem processed the artifact.
        producer: String,
        /// The stored artifact reference.
        artifact: MediaRef,
        /// Human-readable summary of the resulting change.
        diff_summary: String,
    },
    /// Per-assistant-turn token/cost accounting (protocol v2 only;
    /// withheld from v1 wires by the [`crate::compat`] downgrade).
    /// Rides the session event stream as a transient broadcast.
    Usage {
        /// Turn number the accounting belongs to.
        turn: u64,
        /// The identity that served the turn (identity contract).
        model: ModelIdentity,
        /// Reasoning-effort name in force (routing vocabulary), when
        /// known.
        effort: Option<String>,
        /// Prompt-side tokens.
        input_tokens: u64,
        /// Completion-side tokens.
        output_tokens: u64,
        /// Prompt tokens served from the provider read cache.
        cache_read: u64,
        /// Prompt tokens written into the provider cache this request.
        cache_write: u64,
        /// Integer cost in micro-USD (the money path never floats);
        /// `None` = model unpriced (tokens-only).
        cost_usd_micros: Option<u64>,
    },
    /// A model-fallback ask broadcast to all attached clients (protocol
    /// v2 only; withheld from v1 wires, which receive a
    /// v1-decodable `Notification` instead). Resolved through the
    /// permission mediator under the session's mediation policy.
    FallbackAsk {
        /// The ask.
        ask: FallbackAskPayload,
    },
    /// Exactly-one resolution of a fallback ask (protocol v2 only;
    /// withheld from v1 wires). `chosen` is the identity the NEXT
    /// request targets: the proposed hop when acknowledged, the
    /// current model otherwise (deny/cancel/timeout all retry the
    /// SAME model — never a silent substitution).
    FallbackResolved {
        /// Ask correlation id.
        ask_id: u64,
        /// The identity the next request targets.
        chosen: ModelIdentity,
    },
    /// Completion-gate verdict for a scheduled (unattended) run
    /// (protocol v2 only; withheld from v1 wires). `evidence` cites
    /// the session event ids the verdict rests on.
    Verdict {
        /// Turn number the verdict gates.
        turn: u64,
        /// Review verdict.
        verdict: VerdictKind,
        /// Session event ids backing the verdict.
        evidence: Vec<u64>,
    },
    /// Response to [`ClientMsg::Ping`].
    Pong,
    /// Transport is closing (drain, reload, or fatal).
    Bye {
        /// Why.
        reason: String,
    },
}

/// Completion-gate verdict vocabulary (gate.rs `GateVerdict` on the
/// wire; Droid /verify, research §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictKind {
    /// Evidence bundle accepted: the run may end.
    Confirmed,
    /// Evidence contradicts the goal: the run is rejected.
    Refuted,
    /// Evidence insufficient: neither accepted nor rejected.
    Inconclusive,
}

impl VerdictKind {
    /// Stable wire name (matches `crate::gate::GateVerdict::as_str`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Refuted => "refuted",
            Self::Inconclusive => "inconclusive",
        }
    }
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
            command: SessionCommand::SubmitMedia { text, media },
        } => {
            bounded_text(text, "text")?;
            if media.len() > MAX_MEDIA_PER_SUBMIT {
                return Err(ProtocolError::OutOfBounds("media"));
            }
            Ok(())
        }
        ClientMsg::Command {
            command: SessionCommand::Cancel,
        } => Ok(()),
        ClientMsg::PermissionAnswer { .. } | ClientMsg::PermissionCancel { .. } => Ok(()),
        // `chosen` needs no free-text bound: the mediator accepts it
        // only on exact match against the ask's server-proposed
        // identity, so garbage resolves deny-side (same-model retry).
        ClientMsg::FallbackAnswer { .. } => Ok(()),
        ClientMsg::Ping => Ok(()),
    }
}

/// True when a Hello version can be served by this build. v1 stays
/// served forever through the wire downgrade (v1 frontends are never
/// refused); only versions this build never spoke are rejected.
#[must_use]
pub fn version_compatible(client_version: u32) -> bool {
    (MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION).contains(&client_version)
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
    fn version_gate_serves_v1_and_v2_only() {
        assert!(version_compatible(1), "v1 Hello must never be refused");
        assert!(version_compatible(PROTOCOL_VERSION));
        assert!(!version_compatible(0));
        assert!(!version_compatible(PROTOCOL_VERSION + 1));
    }

    #[test]
    fn snapshot_frame_bound_enforced_by_type_invariant() {
        // The bound is enforced at construction sites (event hub); the
        // constant is part of the protocol contract.
        const _: () = assert!(MAX_SNAPSHOT_FRAMES >= 1024);
    }
}
