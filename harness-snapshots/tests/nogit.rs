//! NO-GIT FALLBACK: non-repo projects snapshot via content-addressed file
//! snapshots under .harness/snapshots/ with capability degradation, and
//! dirty-state restore still works transactionally.

mod common;

use common::*;
use harness_snapshots::{
    ExternallyModifiedPolicy, RestoreMode, SessionId, SnapshotCapabilities, SnapshotError,
    SnapshotOptions, SnapshotStore,
};
use std::path::Path;

fn plain_project(files: &[(&str, &[u8])]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    for (rel, bytes) in files {
        write_file(&root, Path::new(rel), bytes);
    }
    (dir, root)
}

/// Happy: round-trip rewind without any git; byte-identical restore.
#[test]
fn no_git_round_trip_is_byte_identical() {
    let (_dir, root) = plain_project(&[("a.txt", b"alpha\n"), ("d/b.txt", b"beta\r\n")]);
    let session_root = root.join("session");
    let store = SnapshotStore::open(&root).expect("store");
    assert_eq!(store.backend_name(), "nogit");
    let before = tree_digest(&root);
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");
    write_file(&root, Path::new("a.txt"), b"changed\n");
    write_file(&root, Path::new("new.txt"), b"created\n");
    delete_file(&root, Path::new("d/b.txt"));
    store
        .snapshot(&sess("s1"), turn(2), &SnapshotOptions::default())
        .expect("snapshot@2");

    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    let report = store.execute_rewind(plan, None, None).expect("rewind");
    assert!(!report.rolled_back);
    assert_eq!(tree_digest(&root), before, "byte-identical no-git restore");

    // Snapshots live under .harness/snapshots/, content-addressed.
    let objects = root
        .join(".harness")
        .join("snapshots")
        .join("s1")
        .join("objects");
    assert!(objects.is_dir(), "CAS objects directory");
    let count = std::fs::read_dir(&objects).unwrap().count();
    assert!(
        count >= 2,
        "distinct contents are distinct objects: {count}"
    );
}

/// Capability flags surface the degraded (git-only features off) mode.
#[test]
fn no_git_capabilities_disable_git_only_features() {
    let (_dir, root) = plain_project(&[("a.txt", b"alpha\n")]);
    let store = SnapshotStore::open(&root).expect("store");
    let caps: SnapshotCapabilities = store.capabilities();
    assert!(!caps.refs, "refs are git-only");
    assert!(!caps.provenance, "provenance trailers are git-only");

    let (_gdir, groot) = git_repo(&[("a.txt", b"alpha\n")]);
    let git_store = SnapshotStore::open(&groot).expect("git store");
    let git_caps = git_store.capabilities();
    assert_eq!((git_caps.refs, git_caps.provenance), (true, true));
}

/// Failure + transaction: aborted no-git restore rolls back; a successful
/// restore stashes the dirty state as a CAS manifest under
/// .harness/restore-transactions/<uuid>/.
#[test]
fn no_git_dirty_state_stash_and_rollback() {
    let (_dir, root) = plain_project(&[("a.txt", b"alpha\n"), ("b.txt", b"beta\n")]);
    let session_root = root.join("session");
    let store = SnapshotStore::open(&root).expect("store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");
    write_file(&root, Path::new("a.txt"), b"alpha\ndirty\n");
    write_file(&root, Path::new("b.txt"), b"beta\ndirty\n");
    let dirty = tree_digest(&root);

    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    let mut probe = FailingProbe::at(1);
    let err = store
        .execute_rewind(plan, None, Some(&mut probe))
        .expect_err("abort");
    assert!(
        matches!(err, SnapshotError::RestoreFailed { restored: true, .. }),
        "rollback completed: {err:?}"
    );
    assert_eq!(
        tree_digest(&root),
        dirty,
        "rollback leaves the dirty tree intact"
    );

    let plan = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect("plan");
    let report = store.execute_rewind(plan, None, None).expect("restore");
    let stash = report.stash.as_ref().expect("dirty stash");
    let dir = &stash.ref_name;
    assert!(dir.contains("restore-transactions"), "{dir}");
    assert!(
        root.join(".harness").join("restore-transactions").is_dir(),
        "stash recorded on disk"
    );
}

/// Malformed: a corrupt CAS manifest is typed, never guessed.
#[test]
fn no_git_corrupt_manifest_is_typed() {
    let (_dir, root) = plain_project(&[("a.txt", b"alpha\n")]);
    let session_root = root.join("session");
    let store = SnapshotStore::open(&root).expect("store");
    store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot@1");
    let manifests = root
        .join(".harness")
        .join("snapshots")
        .join("s1")
        .join("manifests");
    let file = std::fs::read_dir(&manifests)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(&file, b"{torn").unwrap();
    let err = store
        .rewind_plan(
            &session_root,
            &sess("s1"),
            turn(1),
            RestoreMode::FilesOnly,
            ExternallyModifiedPolicy::Include,
        )
        .expect_err("torn manifest");
    assert!(
        matches!(
            err,
            SnapshotError::CorruptSnapshot { .. } | SnapshotError::TornSnapshot { .. }
        ),
        "got {err:?}"
    );
    let _ = SessionId::new("x"); // keep the import honest
}
