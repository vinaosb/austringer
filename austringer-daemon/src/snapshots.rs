//! Turn-completion snapshot recording: every finished turn records an
//! austringer-snapshots checkpoint against the session's workspace
//! binding, then maintenance (SnapshotGcPolicy + scratch reap) keeps
//! the store bounded. Recording sits AROUND the engine pipeline — the
//! driver calls it after the turn's events are published and before
//! `TurnFinished`, so a client that saw the finish can `/rewind`
//! immediately. Fail-open WITH NOTICE: a recording failure publishes a
//! `snapshot` Notification and never kills the turn (the
//! no-silent-degradation law).

use std::path::Path;
use std::time::SystemTime;

use austringer_core::event::{ContentBlock, Event, RecordedEvent};
use austringer_snapshots::ids::{SessionId, Turn};
use austringer_snapshots::scratch::{RETENTION, reap_scratch};
use austringer_snapshots::{
    CHECKPOINTS_DIR, CheckpointMeta, CheckpointOrigin, CheckpointRecord, CheckpointRecorder,
    CheckpointWriter, SessionThreadRecorder, SnapshotError, SnapshotGcPolicy, SnapshotOptions,
    SnapshotStore, TranscriptEntry,
};

use crate::attach::SessionHost;
use crate::protocol::SessionEvent;

/// Conservative GC default: the newest 10 restore points per session.
pub(crate) const DEFAULT_SNAPSHOT_KEEP_LAST: usize = 10;

/// One completed turn's recording pass: workspace snapshot, incremental
/// transcript checkpoint (the rewind truncation-cursor source),
/// session-thread record, then GC + scratch reap. The `now` clock
/// parameter exists so maintenance (reap) is testable without
/// backdating filesystem mtimes.
///
/// # Errors
/// Every failure is [`SnapshotError`]; the caller publishes the notice.
pub(crate) fn record_turn(
    workdir: &Path,
    session: &SessionId,
    turn: Turn,
    entries: &[TranscriptEntry],
    keep_last: usize,
    now: SystemTime,
) -> Result<(), SnapshotError> {
    let store = SnapshotStore::open(workdir)?;
    let outcome = store.snapshot(session, turn, &SnapshotOptions::default())?;
    let target = outcome
        .ref_name
        .clone()
        .or_else(|| {
            outcome
                .manifest_path
                .as_ref()
                .map(|path| path.display().to_string())
        })
        .unwrap_or_default();
    CheckpointWriter::new(workdir)?.write(
        session,
        turn,
        entries,
        &CheckpointMeta {
            snapshot: (!target.is_empty()).then(|| target.clone()),
            ..CheckpointMeta::default()
        },
    )?;
    SessionThreadRecorder::new(workdir).record(&CheckpointRecord {
        session: session.clone(),
        turn,
        origin: CheckpointOrigin::Native,
        target,
    })?;
    maintain(&store, session, workdir, keep_last, now)?;
    tracing::debug!(
        session = session.as_str(),
        turn = turn.get(),
        backend = store.backend_name(),
        "turn checkpoint recorded"
    );
    Ok(())
}

/// Post-recording maintenance: policy GC over the store, checkpoint
/// sidecars swept for turns GC dropped, stale scratch dirs reaped.
fn maintain(
    store: &SnapshotStore,
    session: &SessionId,
    workdir: &Path,
    keep_last: usize,
    now: SystemTime,
) -> Result<(), SnapshotError> {
    // Tier 0 protects the newest turns and tier 1 caps the count: the
    // pair is what makes a lone `keep_last` bound storage (tier 0
    // alone only shields the single newest snapshot).
    let policy = SnapshotGcPolicy {
        keep_last_per_session: keep_last,
        max_per_session: Some(keep_last),
        max_age: None,
    };
    store.gc(&policy, &[])?;
    let retained: Vec<u32> = store.turns_of(session)?.iter().map(|t| t.get()).collect();
    sweep_checkpoints(workdir, &retained);
    if let Err(error) = reap_scratch(&store.scratch_root(), now, RETENTION) {
        tracing::warn!(%error, "scratch reap failed");
    }
    Ok(())
}

/// Remove transcript-checkpoint sidecars for turns GC dropped: rewind
/// only resolves targets that still have snapshots, so orphaned
/// sidecars are pure residue. Best-effort by design.
fn sweep_checkpoints(workdir: &Path, retained: &[u32]) {
    let Ok(children) = std::fs::read_dir(workdir.join(CHECKPOINTS_DIR)) else {
        return;
    };
    for entry in children.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(turn) = stem.parse::<u32>()
            && !retained.contains(&turn)
            && let Err(error) = std::fs::remove_file(path)
        {
            tracing::warn!(%error, "checkpoint sweep failed");
        }
    }
}

/// The turn's events as transcript entries (the incremental-checkpoint
/// shape): every event kind participates — the checkpoint is a
/// host-side record, not model context; media blocks render as their
/// typed markers.
pub(crate) fn transcript_entries(events: &[RecordedEvent]) -> Vec<TranscriptEntry> {
    events
        .iter()
        .map(|recorded| {
            let blocks_text = |blocks: &[ContentBlock]| {
                blocks
                    .iter()
                    .map(|block| block.text_marker())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let (role, content) = match &recorded.event {
                Event::User { blocks } => ("user", blocks_text(blocks)),
                Event::Assistant { blocks } => ("assistant", blocks_text(blocks)),
                Event::ToolCall {
                    tool, arguments, ..
                } => ("tool_call", format!("{tool}({arguments})")),
                Event::ToolResult { blocks, .. } => ("tool_result", blocks_text(blocks)),
                Event::System { text } => ("system", text.clone()),
                Event::Condensation { summary, .. } => ("condensation", summary.clone()),
                Event::Snapshot { label } => ("snapshot", label.clone()),
            };
            TranscriptEntry {
                seq: recorded.seq.0,
                role: role.to_owned(),
                content,
            }
        })
        .collect()
}

impl SessionHost {
    /// The engine driver's post-turn seam: record the completed turn's
    /// checkpoint on the blocking pool. Hosts without a session dir
    /// (children built outside the factory) record nothing — honest
    /// absence. A failure (or a panicking recording task) publishes a
    /// `snapshot` Notification; the turn is already done and stays
    /// done.
    pub(crate) async fn record_turn_checkpoint(&self, turn: u64, events: &[RecordedEvent]) {
        let Some(session_dir) = self.media_dir.get().cloned() else {
            return;
        };
        let Some(session) = SessionId::new(self.session_id()).ok() else {
            self.notice_snapshot_failure("session id is not snapshot-addressable");
            return;
        };
        let Some(slot) = u32::try_from(turn).ok().and_then(|n| Turn::new(n).ok()) else {
            self.notice_snapshot_failure("turn number is not snapshot-addressable");
            return;
        };
        let keep_last = *self
            .snapshot_keep_last
            .get()
            .unwrap_or(&DEFAULT_SNAPSHOT_KEEP_LAST);
        let entries = transcript_entries(events);
        let outcome = tokio::task::spawn_blocking(move || {
            record_turn(
                &session_dir,
                &session,
                slot,
                &entries,
                keep_last,
                SystemTime::now(),
            )
        })
        .await;
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(error)) => self.notice_snapshot_failure(&error.to_string()),
            Err(join) => self.notice_snapshot_failure(&format!("recording task failed: {join}")),
        }
    }

    fn notice_snapshot_failure(&self, message: &str) {
        tracing::warn!(session = self.session_id(), "{message}");
        self.publish(SessionEvent::Notification {
            kind: "snapshot".into(),
            message: format!("turn checkpoint recording failed: {message}"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use austringer_core::event::{ContentBlock, Sequence};
    use std::path::PathBuf;
    use std::time::Duration;

    fn recorded(seq: u64, event: Event) -> RecordedEvent {
        RecordedEvent {
            seq: Sequence(seq),
            event,
        }
    }

    fn text(text: &str) -> ContentBlock {
        ContentBlock::Text {
            text: text.to_owned(),
        }
    }

    #[test]
    fn transcript_entries_map_every_event_kind() {
        let events = vec![
            recorded(
                1,
                Event::User {
                    blocks: vec![text("hi")],
                },
            ),
            recorded(
                2,
                Event::Assistant {
                    blocks: vec![text("hello")],
                },
            ),
            recorded(
                3,
                Event::ToolCall {
                    call_id: "c1".into(),
                    tool: "echo".into(),
                    arguments: serde_json::json!({"q": 1}),
                },
            ),
            recorded(
                4,
                Event::ToolResult {
                    call_id: "c1".into(),
                    blocks: vec![text("ok")],
                },
            ),
            recorded(
                5,
                Event::System {
                    text: "notice".into(),
                },
            ),
            recorded(
                6,
                Event::Condensation {
                    summary: "folded".into(),
                    through_seq: Sequence(5),
                },
            ),
            recorded(
                7,
                Event::Snapshot {
                    label: "turn-1".into(),
                },
            ),
        ];
        let entries = transcript_entries(&events);
        let roles: Vec<&str> = entries.iter().map(|e| e.role.as_str()).collect();
        assert_eq!(
            roles,
            vec![
                "user",
                "assistant",
                "tool_call",
                "tool_result",
                "system",
                "condensation",
                "snapshot"
            ]
        );
        assert_eq!(entries[0].content, "hi");
        assert!(entries[2].content.contains("echo"));
        assert_eq!(entries.last().unwrap().seq, 7, "seqs ride along");
    }

    /// The recording pass lands every artifact in one go and reaps a
    /// stale scratch dir at the same maintenance point (the clock is
    /// simulated 15 days ahead, the crate's own reaper-test trick:
    /// Windows mtimes cannot be backdated).
    #[test]
    fn record_turn_snapshots_checkpoints_and_reaps_stale_scratch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workdir = dir.path().join("session-root");
        std::fs::create_dir_all(&workdir).expect("workdir");
        std::fs::write(workdir.join("a.txt"), b"alpha\n").expect("seed");

        let ghost = store_scratch_ghost(&workdir);
        assert!(ghost.join("index").exists(), "ghost scratch pre-exists");

        let session = SessionId::new("s-maint").expect("session");
        let entries = vec![TranscriptEntry {
            seq: 4,
            role: "assistant".into(),
            content: "done".into(),
        }];
        let future = SystemTime::now() + Duration::from_secs(15 * 24 * 60 * 60);
        record_turn(
            &workdir,
            &session,
            Turn::new(1).unwrap(),
            &entries,
            10,
            future,
        )
        .expect("record");

        let store = SnapshotStore::open(&workdir).expect("reopen");
        assert_eq!(
            store.turns_of(&session).expect("turns"),
            vec![Turn::new(1).unwrap()],
            "snapshot manifest recorded"
        );
        let checkpoint: PathBuf = workdir
            .join(CHECKPOINTS_DIR)
            .join(format!("{:010}.json", 1));
        let bytes = std::fs::read(&checkpoint).expect("checkpoint sidecar");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("checkpoint json");
        assert_eq!(json["through_seq"], 4, "cursor from the entries");
        assert!(
            json["meta"]["snapshot"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "sidecar names its snapshot target"
        );
        let thread = std::fs::read_to_string(workdir.join("checkpoint-thread.jsonl"))
            .expect("session thread");
        assert!(thread.contains("\"origin\":\"native\""), "thread: {thread}");
        assert!(
            !ghost.exists(),
            "stale scratch dir must be reaped at the same maintenance point"
        );
    }

    fn store_scratch_ghost(workdir: &Path) -> PathBuf {
        let ghost = workdir
            .join(".austringer")
            .join("scratch")
            .join("crashed-session");
        std::fs::create_dir_all(&ghost).expect("ghost dir");
        std::fs::write(ghost.join("index"), b"DIRC\x00\x00\x00\x02\x00\x00\x00\x00")
            .expect("ghost index");
        ghost
    }
}
