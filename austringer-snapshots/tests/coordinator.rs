//! Periodic workspace checkpoint coordinator (Warp): the safe-boundary
//! gated 5-state machine, the cheap dirty-check, end-of-run snapshots, and
//! rewind-during-rewind serialization.

mod common;

use austringer_snapshots::{
    CheckpointCoordinator, CoordinatorState, ExternallyModifiedPolicy, RestoreMode,
    SessionThreadRecorder, SnapshotOptions, SnapshotStore,
};
use common::*;
use std::path::Path;

fn coordinator(workdir: &Path) -> CheckpointCoordinator {
    let store = SnapshotStore::open(workdir).expect("store");
    CheckpointCoordinator::new(
        store,
        Box::new(SessionThreadRecorder::new(
            &workdir.join(".austringer").join("session"),
        )),
    )
}

/// The 5-state machine: Idle -> Arming (quiet gate) -> Snapshotting ->
/// Armed -> Restoring -> Armed. A tool mid-flight blocks Arming.
#[test]
fn state_machine_gates_on_safe_boundaries() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join(".austringer").join("session");
    let mut coord = coordinator(&workdir);

    assert_eq!(coord.state(), CoordinatorState::Idle);
    coord.on_safe_boundary(false).expect("boundary observed");
    assert_eq!(
        coord.state(),
        CoordinatorState::Idle,
        "tool mid-flight blocks arming"
    );

    coord.on_safe_boundary(true).expect("quiet");
    assert_eq!(coord.state(), CoordinatorState::Arming);

    let outcome = coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(1),
            &SnapshotOptions::default(),
        )
        .expect("checkpoint")
        .expect("took first snapshot");
    assert_eq!(outcome.turn, turn(1));
    assert_eq!(coord.state(), CoordinatorState::Armed);
}

/// AUTO-UPDATE: the cheap dirty-check suppresses redundant checkpoints and
/// re-snapshots exactly when the repo changed since the last look.
#[test]
fn dirty_check_drives_auto_update() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join(".austringer").join("session");
    let mut coord = coordinator(&workdir);
    coord.on_safe_boundary(true).expect("quiet");
    coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(1),
            &SnapshotOptions::default(),
        )
        .expect("first")
        .expect("snapshot taken");

    // Unchanged repo: turn 2 is a no-op (fingerprint identical).
    let second = coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(2),
            &SnapshotOptions::default(),
        )
        .expect("second");
    assert!(second.is_none(), "no repo change, no new snapshot");

    // A file change flips the fingerprint and re-snapshots.
    write_file(&workdir, Path::new("a.txt"), b"alpha\nchanged\n");
    let third = coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(3),
            &SnapshotOptions::default(),
        )
        .expect("third");
    assert!(third.is_some(), "repo changed -> auto-update checkpoint");

    // Fingerprint sensitivity is direct, too.
    let f1 = austringer_snapshots::DirtyCheck::fingerprint(&workdir);
    write_file(&workdir, Path::new("a.txt"), b"alpha\nchanged again\n");
    let f2 = austringer_snapshots::DirtyCheck::fingerprint(&workdir);
    assert_ne!(f1, f2, "cheap stat walk detects the change");
    assert_eq!(
        austringer_snapshots::DirtyCheck::fingerprint(&workdir),
        f2,
        "stable when nothing changes"
    );
}

/// END-OF-RUN snapshots fire regardless of the dirty check.
#[test]
fn end_of_run_snapshots_always_fire() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join(".austringer").join("session");
    let mut coord = coordinator(&workdir);
    let outcome = coord
        .end_of_run(
            &session_root,
            &sess("s1"),
            turn(4),
            &SnapshotOptions::default(),
        )
        .expect("end of run");
    assert_eq!(outcome.turn, turn(4));
    // Even with zero changes since, end-of-run records the boundary.
    let again = coord
        .end_of_run(
            &session_root,
            &sess("s1"),
            turn(5),
            &SnapshotOptions::default(),
        )
        .expect("end of run 2");
    assert_eq!(again.turn, turn(5));
}

/// REPEATED-INTERRUPTIONS: a rewind during a rewind is refused (Busy) —
/// restores serialize through the state machine.
#[test]
fn rewind_during_rewind_is_serialized() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join(".austringer").join("session");
    let mut coord = coordinator(&workdir);
    coord.on_safe_boundary(true).expect("quiet");
    coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(1),
            &SnapshotOptions::default(),
        )
        .expect("cp")
        .expect("snapshot");
    write_file(&workdir, Path::new("a.txt"), b"wandered\n");
    coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(2),
            &SnapshotOptions::default(),
        )
        .expect("cp2")
        .expect("snapshot 2");

    coord.begin_restore().expect("armed -> restoring");
    assert_eq!(coord.state(), CoordinatorState::Restoring);
    let err = coord.begin_restore().expect_err("nested restore refused");
    assert!(
        matches!(err, austringer_snapshots::SnapshotError::Busy { .. }),
        "got {err:?}"
    );
    coord.finish_restore();
    assert_eq!(coord.state(), CoordinatorState::Armed);
    coord.begin_restore().expect("restore again after finish");
}

/// The coordinator drives a full rewind through the store it owns.
#[test]
fn coordinator_rewind_round_trip() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let session_root = workdir.join(".austringer").join("session");
    let mut coord = coordinator(&workdir);
    coord.on_safe_boundary(true).expect("quiet");
    coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(1),
            &SnapshotOptions::default(),
        )
        .expect("cp")
        .expect("snapshot");
    let before = tree_digest(&workdir);
    write_file(&workdir, Path::new("a.txt"), b"wandered\n");
    coord
        .maybe_checkpoint(
            &session_root,
            &sess("s1"),
            turn(2),
            &SnapshotOptions::default(),
        )
        .expect("cp2")
        .expect("snapshot 2");

    coord.begin_restore().expect("restoring");
    let report = coord
        .rewind(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesAndTask,
            ExternallyModifiedPolicy::Include,
        )
        .expect("rewind");
    coord.finish_restore();
    assert!(!report.rolled_back);
    assert_eq!(tree_digest(&workdir), before);
}
