//! Scratch stat-cache (persistent per-session git-index-format file, the
//! Cline PERF pattern), the 14-day reaper, and snapshot GC tiers.

mod common;

use austringer_snapshots::scratch::{ScratchIndex, ScratchStat, reap_scratch};
use austringer_snapshots::{SnapshotGcPolicy, SnapshotOptions, SnapshotStore};
use common::*;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// PERF: the persistent scratch index skips re-hashing unchanged files —
/// the second snapshot reuses every stat entry, one touched file re-hashes
/// exactly one blob.
#[test]
fn scratch_index_avoids_rehashing_unchanged_files() {
    let (_dir, workdir) = git_repo(&[
        ("a.txt", b"alpha\n"),
        ("b.txt", b"beta\n"),
        ("c.txt", b"gamma\n"),
    ]);
    let store = SnapshotStore::open(&workdir).expect("store");
    let first = store
        .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
        .expect("snapshot 1");
    assert_eq!(
        (first.stats.hashed, first.stats.reused),
        (3, 0),
        "cold walk hashes all"
    );

    let second = store
        .snapshot(&sess("s1"), turn(2), &SnapshotOptions::default())
        .expect("snapshot 2");
    assert_eq!(
        (second.stats.hashed, second.stats.reused),
        (0, 3),
        "stat-cache reuses all"
    );

    write_file(&workdir, Path::new("b.txt"), b"beta\nchanged\n");
    let third = store
        .snapshot(&sess("s1"), turn(3), &SnapshotOptions::default())
        .expect("snapshot 3");
    assert_eq!(
        (third.stats.hashed, third.stats.reused),
        (1, 2),
        "one touched file re-hashes"
    );

    // The scratch index is a REAL git index file: stat-cache present.
    let index_path = workdir
        .join(".austringer")
        .join("scratch")
        .join("s1")
        .join("index");
    let bytes = std::fs::read(&index_path).expect("index file");
    assert_eq!(&bytes[0..4], b"DIRC", "git index format");
}

/// The scratch index round-trips through the git index v2 format; torn and
/// corrupt files are typed.
#[test]
fn scratch_index_round_trip_and_typed_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let scratch = dir.path();
    let mut index = ScratchIndex::default();
    index.record(
        "a.txt",
        ScratchStat {
            mtime_secs: 1_700_000_000,
            mtime_nanos: 123,
            size: 42,
        },
        "0123456789012345678901234567890123456789",
    );
    index.save(scratch).expect("save");
    let loaded = ScratchIndex::open(scratch).expect("open");
    let hit = loaded.lookup(
        "a.txt",
        &ScratchStat {
            mtime_secs: 1_700_000_000,
            mtime_nanos: 123,
            size: 42,
        },
    );
    assert_eq!(
        hit,
        Some("0123456789012345678901234567890123456789".to_owned())
    );
    // Stat drift invalidates the entry (mtime moved).
    assert!(
        loaded
            .lookup(
                "a.txt",
                &ScratchStat {
                    mtime_secs: 1_700_000_001,
                    mtime_nanos: 123,
                    size: 42,
                },
            )
            .is_none()
    );
    // Size drift invalidates too.
    assert!(
        loaded
            .lookup(
                "a.txt",
                &ScratchStat {
                    mtime_secs: 1_700_000_000,
                    mtime_nanos: 123,
                    size: 43,
                },
            )
            .is_none()
    );
    assert!(
        loaded
            .lookup(
                "missing.txt",
                &ScratchStat {
                    mtime_secs: 0,
                    mtime_nanos: 0,
                    size: 0
                }
            )
            .is_none()
    );

    // Torn: truncated index file.
    std::fs::write(scratch.join("index"), &b"DIRC\x00\x00\x00\x02\x00"[..]).unwrap();
    let err = ScratchIndex::open(scratch).expect_err("torn");
    assert!(
        matches!(
            err,
            austringer_snapshots::SnapshotError::ScratchIndex { .. }
        ),
        "got {err:?}"
    );

    // Corrupt: bad magic.
    std::fs::write(scratch.join("index"), b"NOTDIRC!").unwrap();
    let err = ScratchIndex::open(scratch).expect_err("corrupt");
    assert!(
        matches!(
            err,
            austringer_snapshots::SnapshotError::ScratchIndex { .. }
        ),
        "got {err:?}"
    );

    // Absent scratch dir opens as an empty index, not an error.
    std::fs::remove_file(scratch.join("index")).unwrap();
    assert!(
        ScratchIndex::open(scratch)
            .expect("empty")
            .lookup(
                "a",
                &ScratchStat {
                    mtime_secs: 0,
                    mtime_nanos: 0,
                    size: 0,
                }
            )
            .is_none()
    );
}

/// The 14-DAY REAPER collects stale scratch dirs; fresh ones stay. The
/// retention window is a parameter (the production call uses 14 days), so
/// the test drives it without backdating mtimes.
#[test]
fn scratch_reaper_collects_after_fourteen_days() {
    let dir = tempfile::tempdir().unwrap();
    let scratch_root = dir.path().join("scratch");
    for session in ["a", "b", "c"] {
        let sdir = scratch_root.join(session);
        std::fs::create_dir_all(&sdir).unwrap();
        std::fs::write(sdir.join("index"), b"DIRC\x00\x00\x00\x02\x00\x00\x00\x00").unwrap();
    }
    let now = SystemTime::now();
    assert_eq!(
        reap_scratch(&scratch_root, now, Duration::from_secs(14 * 24 * 60 * 60)).expect("reap"),
        0,
        "fresh scratch dirs survive the default window"
    );

    // Windows cannot backdate mtimes from the test, so the clock is
    // simulated instead: a view 15 days in the future sees today's dirs
    // as past the retention window.
    let future = now + Duration::from_secs(15 * 24 * 60 * 60);
    let reaped = reap_scratch(
        &scratch_root,
        future,
        Duration::from_secs(14 * 24 * 60 * 60),
    )
    .expect("reap");
    assert_eq!(reaped, 3, "stale scratch dirs are reaped");
    assert!(!scratch_root.join("a").exists());
}

/// GC: age/count tiers prune old snapshot refs; pinned turns are exempt.
#[test]
fn gc_tiers_respect_pinned_snapshots() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let store = SnapshotStore::open(&workdir).expect("store");
    for t in 1..=6u32 {
        write_file(&workdir, Path::new("a.txt"), format!("v{t}\n").as_bytes());
        store
            .snapshot(&sess("s1"), turn(t), &SnapshotOptions::default())
            .expect("snapshot");
    }
    let policy = SnapshotGcPolicy {
        keep_last_per_session: 2,
        max_per_session: Some(3),
        max_age: None,
    };
    let outcome = store
        .gc(&policy, &[(sess("s1"), turn(1))]) // turn 1 pinned
        .expect("gc");
    assert_eq!(outcome.removed, 2, "turns 2 and 3 fall off the count tier");
    assert_eq!(outcome.pinned_kept, 1);
    assert_eq!(outcome.retained, 4, "turns 1 (pinned), 4, 5, 6 survive");
    // The ref tree proves it: kept turns still resolve, removed ones do not.
    // Both platforms' file APIs accept '/' separators in joined paths.
    for t in [1u32, 4, 5, 6] {
        let name = format!("refs/austringer/snapshots/s1/{t:010}");
        let path = workdir.join(".git").join(name);
        assert!(path.exists(), "turn {t} ref must survive: {path:?}");
    }
    for t in [2u32, 3] {
        let name = format!("refs/austringer/snapshots/s1/{t:010}");
        let path = workdir.join(".git").join(name);
        assert!(!path.exists(), "turn {t} ref must be gone: {path:?}");
    }
}
