//! Malformed-input scenarios (plan todo 21): path containment probes,
//! batch atomicity, hash mismatches, duplicates, and long-path parsing.

mod common;

use common::{extended, hash_bytes, tree_digest, write_fixture_tree};
use harness_vfs::{ShadowEdit, ShadowStore, VfsError};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn traversal_and_absolute_paths_rejected_typed() {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 5);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let user_before = tree_digest(target.path());
    let outside = target.path().parent().expect("parent").join("outside.txt");

    let probes: Vec<PathBuf> = if cfg!(windows) {
        vec![
            PathBuf::from("../escape.txt"),
            PathBuf::from("d0/../../escape.txt"),
            PathBuf::from(r"C:\absolute\path.txt"),
            PathBuf::from(r"\\?\C:\verbatim\path.txt"),
            PathBuf::from(r"\\server\share\unc.txt"),
        ]
    } else {
        vec![
            PathBuf::from("../escape.txt"),
            PathBuf::from("d0/../../escape.txt"),
            PathBuf::from("/absolute/path.txt"),
        ]
    };
    for probe in &probes {
        let err = handle
            .apply(&[ShadowEdit::create(probe.clone(), b"x".to_vec())])
            .expect_err("must reject");
        assert!(
            matches!(err, VfsError::InvalidShadowPath { .. }),
            "{probe:?}: {err:?}"
        );
    }
    assert!(!outside.exists(), "nothing written outside the shadow");
    assert_eq!(tree_digest(target.path()), user_before);
}

#[test]
fn one_bad_path_rejects_whole_batch_atomically() {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 5);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store.create_shadow(target.path()).expect("shadow");

    let good = ShadowEdit::create(PathBuf::from("good.txt"), b"good\n".to_vec());
    let bad = ShadowEdit::create(PathBuf::from("../bad.txt"), b"bad\n".to_vec());
    let err = handle.apply(&[good, bad]).expect_err("batch rejected");
    assert!(matches!(err, VfsError::InvalidShadowPath { .. }));
    assert!(
        !handle.shadow_path().join("good.txt").exists(),
        "no partial application: valid edit in a bad batch is not written"
    );
}

#[test]
fn hash_mismatch_rejects_without_writing() {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 5);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let rel = PathBuf::from(common::fixture_rel(1));

    let wrong = ShadowEdit::replace(rel.clone(), [9; 32], b"replaced\n".to_vec());
    let err = handle.apply(&[wrong]).expect_err("mismatch");
    assert!(matches!(err, VfsError::HashMismatch { .. }), "{err:?}");
    assert_eq!(
        std::fs::read(handle.shadow_path().join(&rel)).expect("unchanged"),
        common::fixture_content(1)
    );
}

#[test]
fn duplicate_edit_paths_rejected() {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 5);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let rel = PathBuf::from(common::fixture_rel(0));
    let hash = hash_bytes(&common::fixture_content(0));

    let edit = ShadowEdit::replace(rel, hash, b"v1\n".to_vec());
    let err = handle.apply(&[edit.clone(), edit]).expect_err("duplicate");
    assert!(matches!(err, VfsError::DuplicateEditPath { .. }), "{err:?}");
}

#[test]
fn empty_edit_batch_is_typed_rejection() {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 5);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let err = handle.apply(&[]).expect_err("empty batch");
    assert!(matches!(err, VfsError::EmptyEdits), "{err:?}");
}

#[test]
fn deep_relative_path_beyond_260_chars_roundtrips() {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 3);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store.create_shadow(target.path()).expect("shadow");

    let segment = "n".repeat(24);
    let mut rel = String::new();
    while rel.len() < 260 {
        rel.push_str(&segment);
        rel.push('/');
    }
    rel.push_str("deep.txt");
    assert!(target.path().join(&rel).to_string_lossy().len() > 260);

    handle
        .apply(&[ShadowEdit::create(PathBuf::from(&rel), b"deep\n".to_vec())])
        .expect("apply deep path");
    handle.commit().expect("commit deep path");

    let full = extended(&target.path().join(&rel));
    assert_eq!(std::fs::read(full).expect("deep file readable"), b"deep\n");
}

#[cfg(unix)]
#[test]
fn symlinks_in_tree_skipped_and_edits_on_them_drift() {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 5);
    std::fs::write(target.path().join("real.txt"), "real\n").expect("real");
    std::os::unix::fs::symlink("real.txt", target.path().join("link.txt")).expect("symlink");
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");

    let mut handle = store.create_shadow(target.path()).expect("shadow");
    assert!(
        !handle
            .shadow_path()
            .join("link.txt")
            .symlink_metadata()
            .is_ok(),
        "symlinks are not materialized into the shadow"
    );

    handle
        .apply(&[ShadowEdit::create(
            PathBuf::from("link.txt"),
            b"x\n".to_vec(),
        )])
        .expect("creates a regular file in the shadow");
    let err = handle
        .commit()
        .expect_err("user tree still has the symlink");
    assert!(
        matches!(
            err,
            VfsError::BaseDrift {
                kind: harness_vfs::DriftKind::Appeared,
                ..
            }
        ),
        "{err:?}"
    );
}
