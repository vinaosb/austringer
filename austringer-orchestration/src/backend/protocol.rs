//! The backend child protocol: bounded single-line JSONL framing
//! (app-server style) — one request line in, one response line out.

/// Maximum accepted line length.
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// One artifact declared by a backend response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArtifactLite {
    /// Artifact path.
    pub path: String,
    /// Content hash.
    pub content_hash: String,
}

/// A prompt request to the backend child.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptRequest {
    /// Request id (echoed in the response).
    pub id: u64,
    /// The task objective.
    pub objective: String,
    /// Model hint, if the backend supports one.
    pub model: Option<String>,
}

/// The backend child's response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptResponse {
    /// Echoed request id.
    pub id: u64,
    /// Whether the child succeeded.
    pub ok: bool,
    /// Result summary.
    pub summary: String,
    /// Artifacts produced.
    pub artifacts: Vec<ArtifactLite>,
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
}

/// Protocol failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// The line is not valid JSON for the frame type.
    #[error("bad json: {detail}")]
    BadJson {
        /// Parse detail.
        detail: String,
    },
    /// The line exceeded the frame cap.
    #[error("oversized line {len} > {max}")]
    OversizedLine {
        /// Observed length.
        len: usize,
        /// The cap.
        max: usize,
    },
}

/// Encodes a request as one line.
pub fn encode_request(request: &PromptRequest) -> String {
    serde_json::to_string(request).unwrap_or_else(|_| "{}".to_string())
}

/// Decodes a response line.
pub fn decode_response(line: &str) -> Result<PromptResponse, ProtocolError> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.len() > MAX_LINE_BYTES {
        return Err(ProtocolError::OversizedLine {
            len: trimmed.len(),
            max: MAX_LINE_BYTES,
        });
    }
    serde_json::from_str(trimmed).map_err(|e| ProtocolError::BadJson {
        detail: e.to_string(),
    })
}
