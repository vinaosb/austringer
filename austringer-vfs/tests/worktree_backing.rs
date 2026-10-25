//! Gix committed-tree worktree backing (plan todo 21): clean-repo backing
//! selection, dirty-worktree demotion, committed-tree isolation, and the
//! reclaimed-ref lifecycle under refs/austringer/reclaimed/.

mod common;

use austringer_vfs::{BackingChoice, ShadowEdit, ShadowStore};
use common::{git_fixture, hash_bytes, tree_digest};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

fn reclaim_ref(shadow_id: &str, commit_hex: &str) -> String {
    format!("refs/austringer/reclaimed/{shadow_id}/{commit_hex}")
}

#[test]
fn clean_repo_backs_with_gix_worktree() {
    let fixture = git_fixture(&[
        ("src/main.rs", b"fn main() {}\n"),
        ("docs/readme.md", b"# readme\n"),
    ]);
    let user_before = tree_digest(&fixture.workdir);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");

    let handle = store
        .create_shadow_choice(&fixture.workdir, BackingChoice::GixWorktree)
        .expect("gix backing");
    let backing = handle.backing();
    assert_eq!(backing.name(), "gix-worktree");
    match backing {
        austringer_vfs::Backing::GixWorktree { commit, .. } => {
            assert_eq!(commit, &fixture.commit_hex, "handle discloses base commit");
        }
        other => panic!("expected GixWorktree backing, got {other:?}"),
    }
    assert_eq!(tree_digest(handle.shadow_path()), user_before);
}

#[test]
fn dirty_worktree_demotes_or_refuses() {
    let fixture = git_fixture(&[("src/main.rs", b"fn main() {}\n")]);
    std::fs::write(fixture.workdir.join("src/main.rs"), b"uncommitted change\n").expect("dirty");

    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");

    let user_before = tree_digest(&fixture.workdir);
    let auto = store.create_shadow(&fixture.workdir).expect("auto backing");
    assert_ne!(
        auto.backing().name(),
        "gix-worktree",
        "dirty worktree must not silently use committed-tree fidelity"
    );
    assert_eq!(tree_digest(auto.shadow_path()), user_before);
    drop(auto);

    let forced = store.create_shadow_choice(&fixture.workdir, BackingChoice::GixWorktree);
    assert!(
        forced.is_err(),
        "forced gix backing on a dirty tree is a typed error"
    );
}

#[test]
fn committed_tree_isolation_edits_never_reach_user_tree() {
    let fixture = git_fixture(&[("src/main.rs", b"fn main() {}\n")]);
    let user_before = tree_digest(&fixture.workdir);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store
        .create_shadow_choice(&fixture.workdir, BackingChoice::GixWorktree)
        .expect("gix backing");

    handle
        .apply(&[ShadowEdit::replace(
            PathBuf::from("src/main.rs"),
            hash_bytes(b"fn main() {}\n"),
            b"fn main() { broken\n".to_vec(),
        )])
        .expect("apply in shadow");
    assert_eq!(
        std::fs::read(handle.shadow_path().join("src/main.rs")).expect("shadow copy"),
        b"fn main() { broken\n"
    );
    assert_eq!(
        tree_digest(&fixture.workdir),
        user_before,
        "user worktree byte-identical while edits live in the shadow"
    );

    handle.commit().expect("commit");
    assert_eq!(
        std::fs::read(fixture.workdir.join("src/main.rs")).expect("user tree updated"),
        b"fn main() { broken\n"
    );
}

#[test]
fn retire_writes_reclaimed_ref_and_gc_removes_it() {
    let fixture = git_fixture(&[("src/main.rs", b"fn main() {}\n")]);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store
        .create_shadow_choice(&fixture.workdir, BackingChoice::GixWorktree)
        .expect("gix backing");
    let shadow_id = handle.id().to_string();
    handle.discard();

    let repo = gix::open(fixture.workdir.join(".git")).expect("reopen fixture repo");
    let ref_name = reclaim_ref(&shadow_id, &fixture.commit_hex);
    let reference = repo
        .find_reference(&ref_name)
        .expect("reclaimed ref exists");
    assert_eq!(reference.id().to_hex().to_string(), fixture.commit_hex);

    let report = store
        .sweep(
            SystemTime::now() + Duration::from_secs(90_000),
            &austringer_vfs::GcPolicy {
                min_age: Duration::from_secs(86_400),
            },
        )
        .expect("sweep");
    assert_eq!(report.deleted, 1);
    assert!(report.bytes_reclaimed > 0);
    assert!(
        repo.find_reference(&ref_name).is_err(),
        "final deletion removes the reclaimed ref too"
    );
}

#[test]
fn gc_grace_keeps_just_retired_gix_shadow() {
    let fixture = git_fixture(&[("src/main.rs", b"fn main() {}\n")]);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let mut handle = store
        .create_shadow_choice(&fixture.workdir, BackingChoice::GixWorktree)
        .expect("gix backing");
    handle.discard();

    let report = store
        .sweep(
            SystemTime::now(),
            &austringer_vfs::GcPolicy {
                min_age: Duration::from_secs(3600),
            },
        )
        .expect("sweep within grace");
    assert_eq!(report.deleted, 0);
    assert_eq!(report.retained, 1);
}
