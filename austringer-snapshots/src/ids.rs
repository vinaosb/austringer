//! Typed identifiers and namespace discipline for the snapshot ref tree.
//!
//! Every ref this crate writes lives under `refs/austringer/snapshots/<
//! session>/<turn>`, `refs/austringer/snapshots/<session>/<turn>.json`
//! (manifest sidecar blob), or `refs/austringer/restore-transactions/<uuid>`.
//! The ids are validated once, at construction, so a hostile session name
//! can never traverse out of the namespace.

use serde::{Deserialize, Serialize};

/// A session identifier safe for ref path components: 1..=64 chars of
/// `[A-Za-z0-9._-]`, not starting with `.`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid session id {0:?}: 1..=64 chars of [A-Za-z0-9._-], no leading dot")]
pub struct InvalidId(pub String);

impl SessionId {
    pub fn new(name: &str) -> Result<Self, InvalidId> {
        let len = name.len();
        let legal = (1..=64).contains(&len)
            && !name.starts_with('.')
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-');
        if legal {
            Ok(Self(name.to_owned()))
        } else {
            Err(InvalidId(name.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A 1-based turn number.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct Turn(u32);

impl Turn {
    pub const FIRST: Self = Self(1);

    pub fn new(n: u32) -> Result<Self, InvalidId> {
        if n == 0 {
            Err(InvalidId("turn 0".to_owned()))
        } else {
            Ok(Self(n))
        }
    }

    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }

    /// Zero-padded lexical form used in ref names and manifest files.
    #[must_use]
    pub fn slot(self) -> String {
        format!("{:010}", self.0)
    }
}

/// Event-history truncation cursor for the rewind seam (the T4 log layer
/// owns the real `Sequence`; this mirrors it without a core dependency).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct EventCursor(pub u64);

/// The typed contract `/rewind` hands the session log: truncate history to
/// `turn`, keeping events through `through` (None = the log layer maps the
/// turn boundary itself when no transcript checkpoint recorded seqs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruncationCursor {
    pub session: SessionId,
    pub turn: Turn,
    pub through: Option<EventCursor>,
}

/// Restore-transaction id (UUIDv4-shaped from OS entropy).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxId(pub String);

pub fn new_tx_id() -> Result<TxId, crate::SnapshotError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|source| crate::SnapshotError::Entropy {
        source: std::io::Error::other(source.to_string()),
    })?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(TxId(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )))
}

/// Ref of a workspace snapshot commit.
#[must_use]
pub fn snapshot_ref(session: &SessionId, turn: Turn) -> String {
    format!(
        "refs/austringer/snapshots/{}/{}",
        session.as_str(),
        turn.slot()
    )
}

/// Ref of the manifest sidecar blob for a snapshot.
#[must_use]
pub fn manifest_ref(session: &SessionId, turn: Turn) -> String {
    format!(
        "refs/austringer/snapshots/{}/{}.json",
        session.as_str(),
        turn.slot()
    )
}

/// Ref of a restore-transaction stash.
#[must_use]
pub fn transaction_ref(tx: &TxId) -> String {
    format!("refs/austringer/restore-transactions/{}", tx.0)
}
