//! The protocol error vocabulary: decode/protocol failures and typed
//! on-wire error codes. Extracted from `protocol` to honor the
//! per-file size budget; re-exported there so existing imports are
//! unchanged.

use serde::{Deserialize, Serialize};

/// Decode/protocol failures. All variants are transport-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// Frame exceeded [`crate::protocol::MAX_FRAME_BYTES`].
    #[error("frame exceeds {} bytes", crate::protocol::MAX_FRAME_BYTES)]
    Oversized,
    /// Frame did not decode into a known message.
    #[error("malformed frame: {0}")]
    Malformed(String),
    /// Frame decoded but a field violated its bound.
    #[error("field out of bounds: {0}")]
    OutOfBounds(&'static str),
}

/// Typed error codes on the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Hello version mismatch.
    ProtocolVersion,
    /// Missing/invalid token.
    Auth,
    /// Frame over [`crate::protocol::MAX_FRAME_BYTES`].
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
