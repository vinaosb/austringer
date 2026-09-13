//! Shadow-store GC (plan todo 21, Grok 412GB lesson): one-cycle grace for
//! retired shadows, orphaned active shadows, and disk-usage accounting.

mod common;

use common::{tree_digest, write_fixture_tree};
use harness_vfs::{GcPolicy, ShadowStore};
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

#[test]
fn retired_shadow_survives_one_cycle_then_deleted() {
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 15);
    let reclaimed = root.path().join("reclaimed");

    {
        let mut handle = store.create_shadow(target.path()).expect("shadow");
        handle.discard();
        assert_eq!(
            std::fs::read_dir(&reclaimed)
                .expect("reclaimed dir")
                .count(),
            1,
            "retired shadow archived for grace"
        );
    }

    let within = store
        .sweep(
            SystemTime::now(),
            &GcPolicy {
                min_age: Duration::from_secs(3600),
            },
        )
        .expect("sweep in grace window");
    assert_eq!(within.deleted, 0);
    assert_eq!(within.retained, 1);

    let after = store
        .sweep(
            SystemTime::now() + Duration::from_secs(7200),
            &GcPolicy {
                min_age: Duration::from_secs(3600),
            },
        )
        .expect("sweep past grace");
    assert_eq!(after.deleted, 1);
    assert_eq!(after.retained, 0);
    assert!(
        after.bytes_reclaimed > 0,
        "disk usage accounting surfaces reclaimed bytes"
    );
    assert_eq!(
        std::fs::read_dir(&reclaimed)
            .expect("reclaimed dir")
            .count(),
        0,
        "grace-expired shadow deleted from disk"
    );
}

#[test]
fn orphaned_active_shadow_reaped_after_min_age() {
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 10);

    let handle = store.create_shadow(target.path()).expect("shadow");
    std::mem::forget(handle); // simulate a crashed owner: no Drop, no retire

    let early = store
        .sweep(
            SystemTime::now(),
            &GcPolicy {
                min_age: Duration::from_secs(60),
            },
        )
        .expect("early sweep");
    assert_eq!(early.deleted, 0, "young active shadow survives");

    let late = store
        .sweep(
            SystemTime::now() + Duration::from_secs(120),
            &GcPolicy {
                min_age: Duration::from_secs(60),
            },
        )
        .expect("late sweep");
    assert_eq!(
        late.deleted, 1,
        "orphaned active shadow reaped past min_age"
    );
    assert!(late.bytes_reclaimed > 0);
    assert_eq!(
        std::fs::read_dir(root.path().join("shadows"))
            .expect("shadows dir")
            .count(),
        0
    );
}

#[test]
fn default_policy_is_conservative_24h() {
    assert_eq!(
        GcPolicy::default().min_age,
        Duration::from_secs(24 * 60 * 60)
    );
}

#[test]
fn sweep_repeated_is_idempotent() {
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 5);
    {
        let mut handle = store.create_shadow(target.path()).expect("shadow");
        handle.discard();
    }
    let policy = GcPolicy {
        min_age: Duration::from_secs(1),
    };
    let first = store
        .sweep(SystemTime::now() + Duration::from_secs(5), &policy)
        .expect("first");
    assert_eq!(first.deleted, 1);
    let second = store
        .sweep(SystemTime::now() + Duration::from_secs(5), &policy)
        .expect("second");
    assert_eq!(second.deleted, 0);
    assert_eq!(second.bytes_reclaimed, 0);
}

#[test]
fn committed_shadow_retires_instead_of_leaking_active_slot() {
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), 8);
    let before = tree_digest(target.path());

    let mut handle = store.create_shadow(target.path()).expect("shadow");
    handle
        .apply(&common::fixture_replace_edits(2))
        .expect("apply");
    handle.commit().expect("commit");
    assert_ne!(tree_digest(target.path()), before);
    assert_eq!(
        std::fs::read_dir(root.path().join("shadows"))
            .expect("shadows dir")
            .count(),
        0,
        "committed shadow vacates the active slot"
    );
}
