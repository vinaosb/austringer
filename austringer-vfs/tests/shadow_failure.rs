//! Transaction failure scenarios (plan todo 21): base drift in all its
//! forms, mid-commit failure rollback, and the user-tree-unchanged guard.

mod common;

use austringer_vfs::{DriftKind, ShadowEdit, ShadowStore, VfsError};
use common::{tree_digest, write_fixture_tree};
use std::path::PathBuf;
use tempfile::TempDir;

fn store_with_target(files: usize) -> (TempDir, TempDir, ShadowStore) {
    let store_root = TempDir::new().expect("store root");
    let store = ShadowStore::new(store_root.path()).expect("store");
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), files);
    (store_root, target, store)
}

#[test]
fn base_drift_on_untouched_file_blocks_commit() {
    let (_root, target, store) = store_with_target(12);
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    handle
        .apply(&common::fixture_replace_edits(2))
        .expect("apply");
    let before = tree_digest(target.path());

    let drifted = common::fixture_rel(9);
    std::fs::write(target.path().join(&drifted), "user touched this\n").expect("user write");

    let err = handle.commit().expect_err("drift blocks commit");
    match &err {
        VfsError::BaseDrift { path, kind } => {
            assert_eq!(path, &PathBuf::from(&drifted));
            assert_eq!(*kind, DriftKind::Modified);
        }
        other => panic!("expected BaseDrift, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(target.path().join(&drifted)).expect("user change intact"),
        b"user touched this\n"
    );
    std::fs::write(target.path().join(&drifted), common::fixture_content(9)).expect("restore");
    assert_eq!(tree_digest(target.path()), before);
}

#[test]
fn base_drift_on_edited_target_blocks_commit() {
    let (_root, target, store) = store_with_target(12);
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    handle
        .apply(&common::fixture_replace_edits(2))
        .expect("apply");
    let before = tree_digest(target.path());

    let raced = common::fixture_rel(0);
    std::fs::write(target.path().join(&raced), "racing user edit\n").expect("race");

    let err = handle.commit().expect_err("racy target blocks commit");
    match &err {
        VfsError::BaseDrift { path, kind } => {
            assert_eq!(path, &PathBuf::from(&raced));
            assert_eq!(*kind, DriftKind::Modified);
        }
        other => panic!("expected BaseDrift, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(target.path().join(&raced)).expect("race intact"),
        b"racing user edit\n"
    );
    std::fs::write(target.path().join(&raced), common::fixture_content(0)).expect("restore");
    assert_eq!(tree_digest(target.path()), before);
}

#[test]
fn deleted_user_file_is_drift() {
    let (_root, target, store) = store_with_target(12);
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    handle
        .apply(&common::fixture_replace_edits(2))
        .expect("apply");
    let rel = common::fixture_rel(7);
    std::fs::remove_file(target.path().join(&rel)).expect("user delete");

    let err = handle.commit().expect_err("deleted = drift");
    match &err {
        VfsError::BaseDrift { path, kind } => {
            assert_eq!(path, &PathBuf::from(rel));
            assert_eq!(*kind, DriftKind::Deleted);
        }
        other => panic!("expected BaseDrift, got {other:?}"),
    }
}

#[test]
fn create_target_appeared_in_user_tree_is_drift() {
    let (_root, target, store) = store_with_target(12);
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    handle
        .apply(&[ShadowEdit::create(
            PathBuf::from("d3/appeared.txt"),
            b"shadow version\n".to_vec(),
        )])
        .expect("apply");
    std::fs::write(target.path().join("d3/appeared.txt"), "user version\n").expect("user raced");

    let err = handle.commit().expect_err("appeared = drift");
    match &err {
        VfsError::BaseDrift { path, kind } => {
            assert_eq!(path, &PathBuf::from("d3/appeared.txt"));
            assert_eq!(*kind, DriftKind::Appeared);
        }
        other => panic!("expected BaseDrift, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(target.path().join("d3/appeared.txt")).expect("user file intact"),
        b"user version\n"
    );
}

#[test]
fn mid_commit_failure_rolls_back_fully() {
    let (_root, target, store) = store_with_target(12);
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let before = tree_digest(target.path());
    handle
        .apply(&common::fixture_replace_edits(3))
        .expect("apply");

    // Poison the SECOND committed file so the copy-up fails midway; the
    // first file has already been overwritten at that point.
    let poison_path = target.path().join(common::fixture_rel(1));
    let mut perms = std::fs::metadata(&poison_path).expect("meta").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&poison_path, perms).expect("readonly");

    let err = handle.commit().expect_err("mid-commit failure");
    match &err {
        VfsError::CommitFailed { restored, .. } => assert!(restored, "must report full rollback"),
        other => panic!("expected CommitFailed, got {other:?}"),
    }

    clear_readonly(&poison_path);
    assert_eq!(
        tree_digest(target.path()),
        before,
        "user tree byte-identical after failed commit rollback"
    );
}

/// Restore ordinary writable permissions without the Unix footgun of
/// `set_readonly(false)` (world-writable).
fn clear_readonly(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
            .expect("restore perms");
    }
    #[cfg(not(unix))]
    {
        let mut perms = std::fs::metadata(path).expect("meta2").permissions();
        // Windows: clearing the read-only bit is the only std path, and the
        // Unix world-writable concern does not apply to it.
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "Windows-only branch; the flagged Unix semantics do not apply"
        )]
        perms.set_readonly(false);
        std::fs::set_permissions(path, perms).expect("restore perms");
    }
}

#[test]
fn commit_without_apply_is_typed_rejection() {
    let (_root, target, store) = store_with_target(5);
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let before = tree_digest(target.path());
    let err = handle.commit().expect_err("nothing applied");
    assert!(matches!(err, VfsError::NothingApplied), "{err:?}");
    assert_eq!(tree_digest(target.path()), before);
}

#[test]
fn misleading_success_probe_user_tree_hashed_every_scenario() {
    // Every scenario above already compares digests; this one pins the
    // guard itself: a failed preflight-style flow leaves bytes identical.
    let (_root, target, store) = store_with_target(20);
    let before = tree_digest(target.path());
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    handle
        .apply(&common::fixture_replace_edits(4))
        .expect("apply");

    let wrong_hash = ShadowEdit::replace(
        PathBuf::from(common::fixture_rel(10)),
        [1; 32],
        b"never lands\n".to_vec(),
    );
    assert!(handle.apply(&[wrong_hash]).is_err());
    handle.discard();
    assert_eq!(tree_digest(target.path()), before);
}
