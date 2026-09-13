//! Commit provenance (Amp) and workflow-approval markers.
//!
//! Agent-authored commits carry a session-id git trailer, Co-authored-by
//! attribution, and a workflow-approval marker trailer so CI can require a
//! human approval before agent workflows run. Signing is DETACHED: keys
//! live outside agent processes behind [`SigningService`]; the real
//! implementation is daemon-side and wired later.

use crate::{SessionId, SnapshotError, Turn};

pub const SESSION_TRAILER: &str = "Harness-Session-Id";
pub const AGENT_MARKER_TRAILER: &str = "Harness-Agent-Commit";
pub const AGENT_EMAIL: &str = "agent@harness.invalid";
const AGENT_NAME: &str = "harness-agent";

/// The canonical provenance block for a harness-authored commit: subject
/// line, blank line, then the trailer paragraph.
#[must_use]
pub fn provenance_message(session: &SessionId, turn: Turn) -> String {
    format!(
        "harness snapshot {} turn {}\n\n{SESSION_TRAILER}: {}\nHarness-Turn: {}\n\
         Co-authored-by: {AGENT_NAME} <{AGENT_EMAIL}>\n{AGENT_MARKER_TRAILER}: true\n",
        session.as_str(),
        turn.get(),
        session.as_str(),
        turn.get(),
    )
}

/// The session id recorded in the trailer paragraph, if any.
#[must_use]
pub fn session_trailer(message: &str) -> Option<String> {
    let prefix = format!("{SESSION_TRAILER}: ");
    message.lines().rev().find_map(|line| {
        line.starts_with(&prefix)
            .then(|| line[prefix.len()..].trim().to_owned())
    })
}

/// Whether the commit was authored by the harness agent.
#[must_use]
pub fn is_agent_commit(message: &str) -> bool {
    message
        .lines()
        .any(|line| line.trim() == format!("{AGENT_MARKER_TRAILER}: true"))
}

/// CI gate predicate: agent-authored artifacts require human approval
/// before write-access workflows run.
#[must_use]
pub fn requires_human_approval(message: &str) -> bool {
    is_agent_commit(message)
}

/// A detached signature over an object payload (hex-encoded).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct DetachedSignature {
    pub algorithm: String,
    pub value: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
#[error("signing service failed: {0}")]
pub struct SigningError(pub String);

impl From<SigningError> for SnapshotError {
    fn from(source: SigningError) -> Self {
        SnapshotError::SigningFailed {
            source: Box::new(source),
        }
    }
}

/// Signs payloads with key material that NEVER enters an agent process.
/// The production implementation is a daemon-side service; tests and the
/// wired-later seam use [`MockSigningService`].
pub trait SigningService: Send + Sync {
    fn sign(&self, payload: &[u8]) -> Result<DetachedSignature, SigningError>;
}

/// Keyed-blake3 mock: deterministic, payload-sensitive, one-way.
pub struct MockSigningService {
    key: [u8; 32],
}

impl MockSigningService {
    #[must_use]
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }
}

impl SigningService for MockSigningService {
    fn sign(&self, payload: &[u8]) -> Result<DetachedSignature, SigningError> {
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        hasher.update(payload);
        let hex = hasher.finalize().to_hex().to_string();
        Ok(DetachedSignature {
            algorithm: "blake3-keyed-mock".to_owned(),
            value: hex.into_bytes(),
        })
    }
}

/// Signs a commit object's serialized bytes through an optional service.
pub(crate) fn sign_commit(
    signer: Option<&std::sync::Arc<dyn SigningService>>,
    commit: &gix::objs::Commit,
) -> Result<Option<DetachedSignature>, SnapshotError> {
    let Some(signer) = signer else {
        return Ok(None);
    };
    let mut payload = Vec::new();
    gix::objs::WriteTo::write_to(commit, &mut payload)
        .map_err(|source| SnapshotError::git("serialize commit", source))?;
    Ok(Some(signer.sign(&payload)?))
}
