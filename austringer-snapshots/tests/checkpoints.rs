//! Incremental checkpoint writes (Cursor): only NEW transcript entries per
//! checkpoint, resume-from-last-checkpoint, torn manifests typed, and the
//! host-side CheckpointRecorder (Zed) surface.

mod common;

use austringer_snapshots::{
    CheckpointMeta, CheckpointOrigin, CheckpointRecord, CheckpointRecorder, CheckpointWriter,
    SnapshotError, TranscriptEntry,
};
use common::*;
use std::path::Path;

fn entry(seq: u64, text: &str) -> TranscriptEntry {
    TranscriptEntry {
        seq,
        role: "assistant".into(),
        content: text.into(),
    }
}

fn writer(session_root: &Path) -> CheckpointWriter {
    CheckpointWriter::new(session_root).expect("writer")
}

/// HAPPY: each checkpoint file carries only that turn's new entries — the
/// full conversation is never rewritten (sizes prove it).
#[test]
fn checkpoints_persist_only_new_entries() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let w = writer(root);

    let p1 = w
        .write(
            &sess("s1"),
            turn(1),
            &[entry(1, "hello"), entry(2, "world")],
            &CheckpointMeta::default(),
        )
        .expect("cp1");
    let p2 = w
        .write(
            &sess("s1"),
            turn(2),
            &[entry(3, "third turn only")],
            &CheckpointMeta::default(),
        )
        .expect("cp2");
    let p3 = w
        .write(
            &sess("s1"),
            turn(3),
            &[entry(4, "x")],
            &CheckpointMeta::default(),
        )
        .expect("cp3");
    assert_ne!(p1, p2);
    assert_ne!(p2, p3);

    let s1 = std::fs::metadata(&p1).unwrap().len();
    let s2 = std::fs::metadata(&p2).unwrap().len();
    let s3 = std::fs::metadata(&p3).unwrap().len();
    assert!(s2 < s1, "turn 2 wrote only its new entry ({s2} < {s1})");
    assert!(s3 < s2, "turn 3 wrote only its new entry ({s3} < {s2})");

    let files = std::fs::read_dir(root.join("checkpoints")).unwrap().count();
    assert_eq!(files, 3, "one file per checkpoint, nothing rewritten");
}

/// Resume: subagents pick up from their LAST checkpoint after failure.
#[test]
fn subagents_resume_from_last_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let w = writer(root);
    assert!(w.load(&sess("s1")).expect("empty").is_none());
    w.write(
        &sess("s1"),
        turn(1),
        &[entry(1, "one")],
        &CheckpointMeta::default(),
    )
    .expect("cp1");
    w.write(
        &sess("s1"),
        turn(5),
        &[entry(9, "nine"), entry(10, "ten")],
        &CheckpointMeta::default(),
    )
    .expect("cp5");

    let resume = w.load(&sess("s1")).expect("load").expect("resume point");
    assert_eq!(resume.turn, turn(5));
    assert_eq!(resume.entries.len(), 2);
    assert_eq!(resume.entries[0].seq, 9);
    assert_eq!(resume.entries[1].content, "ten");
    assert_eq!(resume.through_seq, 10);

    // Sessions are isolated.
    assert!(w.load(&sess("other")).expect("load").is_none());
}

/// FAILURE: torn/corrupt checkpoint manifests are typed, never guessed.
#[test]
fn torn_and_corrupt_manifests_are_typed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let w = writer(root);
    w.write(
        &sess("s1"),
        turn(1),
        &[entry(1, "one")],
        &CheckpointMeta::default(),
    )
    .expect("cp1");
    let cp_dir = root.join("checkpoints");
    let file = std::fs::read_dir(&cp_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    let json = std::fs::read_to_string(&file).unwrap();
    // Torn: truncated mid-JSON.
    std::fs::write(&file, &json[..json.len() - 3]).unwrap();
    let err = w.load(&sess("s1")).expect_err("torn");
    assert!(
        matches!(err, SnapshotError::TornCheckpoint { .. }),
        "got {err:?}"
    );

    // Corrupt: valid JSON, wrong shape.
    std::fs::write(&file, "{\"nonsense\": true}").unwrap();
    let err = w.load(&sess("s1")).expect_err("corrupt");
    assert!(
        matches!(err, SnapshotError::CorruptCheckpoint { .. }),
        "got {err:?}"
    );

    // Restored to a valid state, loads work again.
    std::fs::write(&file, &json).unwrap();
    assert!(w.load(&sess("s1")).expect("load").is_some());
}

/// HOST-SIDE (Zed): the host records checkpoints in the session thread,
/// native AND external agents uniformly.
#[test]
fn host_recorder_is_uniform_for_native_and_external_agents() {
    let dir = tempfile::tempdir().unwrap();
    let mut recorder = austringer_snapshots::SessionThreadRecorder::new(dir.path());
    recorder
        .record(&CheckpointRecord {
            session: sess("s1"),
            turn: turn(1),
            origin: CheckpointOrigin::Native,
            target: "refs/austringer/snapshots/s1/0000000001".into(),
        })
        .expect("record native");
    recorder
        .record(&CheckpointRecord {
            session: sess("s1"),
            turn: turn(2),
            origin: CheckpointOrigin::External {
                agent: "zed-acp".into(),
            },
            target: "refs/austringer/snapshots/s1/0000000002".into(),
        })
        .expect("record external");
    let thread =
        std::fs::read_to_string(dir.path().join("checkpoint-thread.jsonl")).expect("thread");
    let lines: Vec<&str> = thread.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"origin\":\"native\""));
    assert!(lines[1].contains("\"origin\":\"external\""));
    assert!(lines[1].contains("zed-acp"));
}
