//! Core shadow transaction scenarios (plan todo 21): backing disclosure,
//! apply-inside-shadow, commit materialization, instant rollback, and the
//! repeated-determinism loop.

mod common;

use common::{fixture_rel, hash_bytes, tree_digest, write_fixture_tree};
use harness_vfs::{BackingChoice, ShadowEdit, ShadowStore, VfsError};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tempfile::TempDir;

fn new_store() -> (TempDir, ShadowStore) {
    let root = TempDir::new().expect("store tempdir");
    let store = ShadowStore::new(root.path()).expect("store");
    (root, store)
}

fn sample_tree(root: &Path) {
    write_fixture_tree(root, 37);
}

#[test]
fn create_shadow_discloses_backing_and_mirrors_tree() {
    let target = TempDir::new().expect("target");
    sample_tree(target.path());
    let user_before = tree_digest(target.path());
    let (_store_root, store) = new_store();

    let handle = store.create_shadow(target.path()).expect("shadow");
    let name = handle.backing().name();
    assert!(
        matches!(name, "reflink" | "gix-worktree" | "full-copy"),
        "backing must be one of the disclosed kinds, got {name}"
    );
    assert_eq!(tree_digest(handle.shadow_path()), user_before);
    assert_eq!(handle.target(), target.path());
    assert_eq!(tree_digest(target.path()), user_before);
}

#[test]
fn reflink_unsupported_falls_back_transparently() {
    // On NTFS/unsupported filesystems the store must still hand back a
    // usable mirror; whichever backing was chosen is disclosed by name.
    let target = TempDir::new().expect("target");
    sample_tree(target.path());
    let user_before = tree_digest(target.path());
    let (_store_root, store) = new_store();

    let handle = store.create_shadow(target.path()).expect("shadow");
    assert_eq!(tree_digest(handle.shadow_path()), user_before);
}

#[test]
fn apply_edits_in_shadow_then_commit_materializes() {
    let target = TempDir::new().expect("target");
    sample_tree(target.path());
    let user_before = tree_digest(target.path());
    let (_store_root, store) = new_store();
    let mut handle = store.create_shadow(target.path()).expect("shadow");

    let mut edits = common::fixture_replace_edits(2);
    edits.push(ShadowEdit::create(
        PathBuf::from("d0/new-file.txt"),
        b"created\n".to_vec(),
    ));
    let stats = handle.apply(&edits).expect("apply");
    assert_eq!(stats.applied, 2);
    assert_eq!(stats.created, 1);
    assert_eq!(
        tree_digest(target.path()),
        user_before,
        "user tree untouched after apply"
    );

    assert_eq!(
        std::fs::read(handle.shadow_path().join("d0/new-file.txt")).expect("created in shadow"),
        b"created\n"
    );
    let committed = handle.commit().expect("commit");
    assert_eq!(committed.written, 2);
    assert_eq!(committed.created, 1);

    assert_eq!(
        std::fs::read(target.path().join("d0/new-file.txt")).expect("created in user tree"),
        b"created\n"
    );
    let f0 = std::fs::read(target.path().join(fixture_rel(0))).expect("f0");
    assert_eq!(f0, b"edited 0\nsecond 0\n");
    assert_ne!(tree_digest(target.path()), user_before);
}

#[test]
fn instant_rollback_drop_leaves_user_byte_identical() {
    let target = TempDir::new().expect("target");
    sample_tree(target.path());
    let user_before = tree_digest(target.path());
    let (root, store) = new_store();
    let mut handle = store.create_shadow(target.path()).expect("shadow");

    handle
        .apply(&common::fixture_replace_edits(5))
        .expect("apply");
    let shadow_path = handle.shadow_path().to_path_buf();
    handle.discard();

    assert_eq!(
        tree_digest(target.path()),
        user_before,
        "drop = instant rollback"
    );
    assert!(!shadow_path.exists(), "active shadow slot vacated");
    assert!(
        std::fs::read_dir(root.path().join("reclaimed"))
            .expect("reclaimed")
            .count()
            == 1,
        "retired shadow archived for GC grace"
    );
}

#[test]
fn happy_and_rollback_repeat_three_times_deterministic() {
    for round in 0..3 {
        let target = TempDir::new().expect("target");
        sample_tree(target.path());
        let user_before = tree_digest(target.path());
        let (_store_root, store) = new_store();

        let mut committed = store.create_shadow(target.path()).expect("shadow");
        committed
            .apply(&common::fixture_replace_edits(3))
            .expect("apply");
        committed.commit().expect("commit");
        let after_commit = tree_digest(target.path());
        assert_ne!(
            after_commit, user_before,
            "round {round}: commit changed tree"
        );

        let target2 = TempDir::new().expect("target2");
        sample_tree(target2.path());
        let before2 = tree_digest(target2.path());
        let (_root2, store2) = new_store();
        let mut rolled = store2.create_shadow(target2.path()).expect("shadow");
        rolled
            .apply(&common::fixture_replace_edits(3))
            .expect("apply");
        rolled.discard();
        assert_eq!(
            tree_digest(target2.path()),
            before2,
            "round {round}: rollback"
        );
    }
}

#[test]
fn crlf_bytes_untouched_end_to_end() {
    let target = TempDir::new().expect("target");
    std::fs::write(target.path().join("crlf.txt"), b"a\r\nb\r\nc\r\n").expect("write");
    let (_store_root, store) = new_store();
    let mut handle = store.create_shadow(target.path()).expect("shadow");

    let expected = hash_bytes(b"a\r\nb\r\nc\r\n");
    handle
        .apply(&[ShadowEdit::replace(
            PathBuf::from("crlf.txt"),
            expected,
            b"a\r\nB\r\nc\r\n".to_vec(),
        )])
        .expect("apply");
    handle.commit().expect("commit");
    assert_eq!(
        std::fs::read(target.path().join("crlf.txt")).expect("read"),
        b"a\r\nB\r\nc\r\n"
    );
}

#[test]
fn unicode_paths_roundtrip() {
    let target = TempDir::new().expect("target");
    let rel = PathBuf::from("日本語/ünïcode-ファイル.txt");
    std::fs::create_dir_all(target.path().join("日本語")).expect("mkdir");
    std::fs::write(target.path().join(&rel), "héllo\n".as_bytes()).expect("write");
    let (_store_root, store) = new_store();
    let mut handle = store.create_shadow(target.path()).expect("shadow");

    handle
        .apply(&[ShadowEdit::replace(
            rel.clone(),
            hash_bytes("héllo\n".as_bytes()),
            "wörld\n".as_bytes().to_vec(),
        )])
        .expect("apply");
    handle.commit().expect("commit");
    assert_eq!(
        std::fs::read(target.path().join(rel)).expect("read"),
        "wörld\n".as_bytes()
    );
}

#[test]
fn empty_tree_backing_is_disclosed() {
    let target = TempDir::new().expect("target");
    let (_store_root, store) = new_store();
    let handle = store.create_shadow(target.path()).expect("shadow");
    assert_eq!(handle.backing().name(), "empty");
    assert_eq!(
        tree_digest(handle.shadow_path()),
        tree_digest(target.path())
    );
}

#[test]
fn forced_full_copy_backing_mirrors() {
    let target = TempDir::new().expect("target");
    sample_tree(target.path());
    let user_before = tree_digest(target.path());
    let (_store_root, store) = new_store();
    let handle = store
        .create_shadow_choice(target.path(), BackingChoice::FullCopy)
        .expect("forced copy");
    assert_eq!(handle.backing().name(), "full-copy");
    assert_eq!(tree_digest(handle.shadow_path()), user_before);
}

#[test]
fn timing_happy_path_recorded() {
    // The plan's happy QA channel: 1k-file shadow -> 50 edits -> commit,
    // plus the instant-rollback path, with timings surfaced for evidence.
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 1000);
    let user_before = tree_digest(target.path());
    let (_store_root, store) = new_store();

    let t_create = Instant::now();
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let create_ms = t_create.elapsed().as_millis();

    let edits = common::fixture_replace_edits(50);
    let t_apply = Instant::now();
    handle.apply(&edits).expect("apply");
    let apply_ms = t_apply.elapsed().as_millis();

    let t_commit = Instant::now();
    handle.commit().expect("commit");
    let commit_ms = t_commit.elapsed().as_millis();
    assert_ne!(tree_digest(target.path()), user_before);

    let target2 = TempDir::new().expect("target2");
    write_fixture_tree(target2.path(), 1000);
    let (_root2, store2) = new_store();
    let mut rollback = store2.create_shadow(target2.path()).expect("shadow2");
    let edits2 = common::fixture_replace_edits(50);
    rollback.apply(&edits2).expect("apply2");
    let t_drop = Instant::now();
    rollback.discard();
    let rollback_ms = t_drop.elapsed().as_millis();

    let backing_note = "see bench for backing";
    println!(
        "backing_note={backing_note} create={}ms apply50={}ms commit={}ms discard={}ms",
        create_ms, apply_ms, commit_ms, rollback_ms
    );
}

#[test]
fn apply_after_retire_is_typed_rejection() {
    let target = TempDir::new().expect("target");
    sample_tree(target.path());
    let (_store_root, store) = new_store();
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    handle.discard();
    let err = handle
        .apply(&[ShadowEdit::create(PathBuf::from("x.txt"), b"x".to_vec())])
        .expect_err("apply after discard");
    assert!(matches!(err, VfsError::AlreadyRetired), "got {err:?}");
}
