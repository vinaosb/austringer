//! The explicit `/rewind` bridge (plan todo 5): `ClientMsg::Rewind`
//! restores the session to an austringer-snapshots restore point and
//! truncates the persisted session log to the restore head. Restore is
//! EXPLICIT-ONLY — nothing in the daemon ever calls into this module
//! except the client-protocol dispatch arms.
//!
//! Scoping today: the snapshot store and the persisted log both hang
//! off the session's own root (`{data}/sessions/{sid}` — the store is
//! session-scoped and hermetic). The loop wiring (todos 21/23)
//! relocates the store to the session's real workspace binding.

use std::path::PathBuf;
use std::sync::Arc;

use austringer_core::event::{Event, Sequence};
use austringer_sessions::SessionError;
use austringer_sessions::SessionStore;
use austringer_sessions::lease::LeaseOwner;
use austringer_snapshots::ids::{InvalidId, SessionId, TruncationCursor};
use austringer_snapshots::{
    ExternallyModifiedPolicy, RestoreMode, SnapshotError, SnapshotStore, Turn,
};

use crate::attach::SessionHost;
use crate::protocol::SessionEvent;

/// Typed rewind refusals and failures. Every request-shaped variant
/// names what the client asked for and what exists.
#[derive(Debug, thiserror::Error)]
pub enum RewindError {
    /// The session host has no materialized session directory.
    #[error("session has no directory; cannot rewind")]
    NoSessionDir,
    /// Zero steps is not a rewind.
    #[error("rewind steps must be at least 1, got {steps}")]
    InvalidSteps { steps: u64 },
    /// The session id cannot address the snapshot namespace.
    #[error("invalid session id for snapshots: {0}")]
    BadSessionId(#[from] InvalidId),
    /// The session has no snapshots at all.
    #[error("no snapshots recorded for session {session}; nothing to rewind")]
    NoSnapshots { session: String },
    /// The request walks past the oldest snapshot; the oldest is named.
    #[error(
        "rewind {requested} exceeds the {available} available snapshot(s); the oldest restore point is turn {oldest}"
    )]
    BeyondOldest {
        requested: u64,
        available: usize,
        oldest: u32,
    },
    /// The snapshot names a turn the persisted log does not have.
    #[error("session log has no turn {turn}; cannot map the restore head")]
    TurnMissingFromLog { turn: u32 },
    /// The session store failed (open, lease, truncate).
    #[error("session log: {0}")]
    Session(#[from] SessionError),
    /// The snapshot transaction failed (plan or restore).
    #[error("snapshot restore: {0}")]
    Snapshot(#[from] SnapshotError),
}

impl RewindError {
    /// Wire mapping for the transport error lane: request-shaped
    /// refusals keep the malformed-frame convention the command lane
    /// already uses; server-side failures are internal.
    #[must_use]
    pub fn error_code(&self) -> crate::protocol::ErrorCode {
        match self {
            Self::NoSessionDir | Self::Session(_) | Self::Snapshot(_) => {
                crate::protocol::ErrorCode::Internal
            }
            Self::InvalidSteps { .. }
            | Self::BadSessionId(_)
            | Self::NoSnapshots { .. }
            | Self::BeyondOldest { .. }
            | Self::TurnMissingFromLog { .. } => crate::protocol::ErrorCode::MalformedFrame,
        }
    }
}

/// Run one explicit rewind for `host`: resolve the target restore
/// point `steps` back, restore workspace files through the snapshot
/// transaction, truncate the persisted log to the restore head, and
/// publish the System "restored to <label>" event on the session's
/// normal event stream (every attached client sees it). Ordering
/// matches the crate's restore transaction: files commit first; a
/// later log-truncation failure surfaces typed with the files already
/// restored (the dirty state stays recoverable through the stash ref).
///
/// # Errors
/// Every failure is [`RewindError`].
pub fn rewind(host: &Arc<SessionHost>, steps: u64) -> Result<(), RewindError> {
    let session_dir: PathBuf = host
        .media_dir
        .get()
        .cloned()
        .ok_or(RewindError::NoSessionDir)?;
    let sid = host.session_id().to_owned();
    if steps == 0 {
        return Err(RewindError::InvalidSteps { steps });
    }
    let session = SessionId::new(&sid)?;
    let snapshots = SnapshotStore::open(&session_dir)?;
    let mut turns = snapshots.turns_of(&session)?;
    if turns.is_empty() {
        return Err(RewindError::NoSnapshots { session: sid });
    }
    turns.sort_unstable();
    let target = select_turn(&turns, steps).ok_or(RewindError::BeyondOldest {
        requested: steps,
        available: turns.len(),
        oldest: turns[0].get(),
    })?;

    let plan = snapshots.rewind_plan(
        &session_dir,
        &session,
        target,
        RestoreMode::FilesOnly,
        ExternallyModifiedPolicy::Skip,
    )?;
    let cursor: TruncationCursor = plan.event_cursor.clone();
    snapshots.execute_rewind(plan, None, None)?;

    // Files are committed; now the log. Head resolution mirrors the
    // crate's RewindLog seam (truncate only after a committed restore)
    // but stays typed — SessionError will not launder through
    // SnapshotError.
    let root = sessions_root(&session_dir);
    let head = Sequence(match cursor.through {
        Some(through) => through.0,
        None => SessionStore::open(&root)?
            .turn_end_seq(&sid, cursor.turn.get())?
            .ok_or(RewindError::TurnMissingFromLog {
                turn: cursor.turn.get(),
            })?,
    });
    let store = SessionStore::open(&root)?;
    let owner = LeaseOwner::new(&format!("austringer-daemon-rewind:{sid}"));
    store.open_session_rw(&sid, &owner)?.truncate_to(head)?;
    host.publish(SessionEvent::Recorded {
        seq: head.0,
        event: Event::System {
            text: format!("restored to turn {}", target.get()),
        },
    });
    Ok(())
}

/// The persisted sessions store for a session root: the daemon data
/// dir, the grandparent of the engine session dir
/// (`{data}/sessions/{sid}` → `{data}`). Shared by the rewind bridge
/// and the event_fts search surface (todo 11).
pub(crate) fn sessions_root(session_dir: &std::path::Path) -> PathBuf {
    session_dir
        .parent()
        .and_then(std::path::Path::parent)
        .map(PathBuf::from)
        .unwrap_or_else(|| session_dir.to_path_buf())
}

/// The async entry the dispatch arms call: the snapshot transaction
/// walks the workspace and touches SQLite, so it runs on the blocking
/// pool.
///
/// # Errors
/// Propagates [`RewindError`] from [`rewind`].
pub async fn run_rewind(host: Arc<SessionHost>, steps: u64) -> Result<(), RewindError> {
    tokio::task::spawn_blocking(move || rewind(&host, steps))
        .await
        .map_err(|join| {
            RewindError::Snapshot(SnapshotError::Serialize(format!(
                "rewind task panicked: {join}"
            )))
        })?
}

/// Turn selection semantics: 1 = the latest snapshot, N = the Nth back
/// (`None` when the list is shorter than N).
#[must_use]
pub fn select_turn(turns: &[Turn], steps: u64) -> Option<Turn> {
    if steps == 0 {
        return None;
    }
    turns
        .len()
        .checked_sub(usize::try_from(steps).unwrap_or(usize::MAX))
        .and_then(|index| turns.get(index).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beyond_oldest_display_names_the_oldest_restore_point() {
        let error = RewindError::BeyondOldest {
            requested: 99,
            available: 3,
            oldest: 1,
        };
        let text = error.to_string();
        assert!(text.contains("oldest restore point is turn 1"), "{text}");
        assert_eq!(
            error.error_code(),
            crate::protocol::ErrorCode::MalformedFrame
        );
    }

    #[test]
    fn select_turn_counts_back_from_the_latest() {
        let turns = vec![
            Turn::new(1).unwrap(),
            Turn::new(2).unwrap(),
            Turn::new(4).unwrap(),
        ];
        assert_eq!(select_turn(&turns, 1), Some(Turn::new(4).unwrap()));
        assert_eq!(select_turn(&turns, 2), Some(Turn::new(2).unwrap()));
        assert_eq!(select_turn(&turns, 3), Some(Turn::new(1).unwrap()));
        assert_eq!(select_turn(&turns, 4), None, "past the oldest");
        assert_eq!(select_turn(&turns, 0), None, "zero steps is not a rewind");
    }
}
