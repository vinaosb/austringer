//! Timing surfaces (plan todo 21 acceptance): apply+rollback on 1k and 10k
//! file trees. Run explicitly: `cargo test -p austringer-vfs --release --test
//! bench -- --ignored --nocapture`. The ≤50 ms apply+rollback budget is
//! asserted only on the reflink/CoW backing; other backings record their
//! numbers honestly instead of failing the platform.

mod common;

use austringer_vfs::ShadowStore;
use common::{tree_digest, write_fixture_tree};
use std::time::Instant;
use tempfile::TempDir;

struct Timings {
    backing: &'static str,
    create_ms: u128,
    apply_ms: u128,
    discard_ms: u128,
}

fn measure(files: usize) -> Timings {
    let target = TempDir::new().expect("target");
    write_fixture_tree(target.path(), files);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");

    let t = Instant::now();
    let mut handle = store.create_shadow(target.path()).expect("shadow");
    let create_ms = t.elapsed().as_millis();
    let backing = handle.backing().name();

    let edits = common::fixture_replace_edits(50);
    let user_before = tree_digest(target.path());
    let t = Instant::now();
    handle.apply(&edits).expect("apply");
    let apply_ms = t.elapsed().as_millis();

    let t = Instant::now();
    handle.discard();
    let discard_ms = t.elapsed().as_millis();

    assert_eq!(
        tree_digest(target.path()),
        user_before,
        "rollback leaves the user tree byte-identical ({files} files)"
    );
    Timings {
        backing,
        create_ms,
        apply_ms,
        discard_ms,
    }
}

#[test]
#[ignore = "benchmark: run with --release -- --ignored --nocapture"]
fn bench_apply_rollback_1k_files() {
    let t = measure(1000);
    println!(
        "1k files: backing={} create={}ms apply50={}ms discard={}ms",
        t.backing, t.create_ms, t.apply_ms, t.discard_ms
    );
    if t.backing == "reflink" {
        assert!(
            t.apply_ms + t.discard_ms <= 50,
            "CoW path budget: apply+rollback {}ms > 50ms",
            t.apply_ms + t.discard_ms
        );
    }
}

#[test]
#[ignore = "benchmark: run with --release -- --ignored --nocapture"]
fn bench_apply_rollback_10k_files() {
    let t = measure(10_000);
    println!(
        "10k files: backing={} create={}ms apply50={}ms discard={}ms",
        t.backing, t.create_ms, t.apply_ms, t.discard_ms
    );
}

#[test]
#[ignore = "benchmark: run with --release -- --ignored --nocapture"]
fn bench_gix_worktree_backing_1k_files() {
    // The committed-tree fallback timing on this machine (NTFS): commits
    // 1k files into a fixture repo, then shadows it via gix.
    let files: Vec<(String, Vec<u8>)> = (0..1000)
        .map(|i| (common::fixture_rel(i), common::fixture_content(i)))
        .collect();
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect();
    let fixture = common::git_fixture(&refs);
    let root = TempDir::new().expect("store root");
    let store = ShadowStore::new(root.path()).expect("store");

    let t = Instant::now();
    let mut handle = store
        .create_shadow_choice(&fixture.workdir, austringer_vfs::BackingChoice::GixWorktree)
        .expect("gix backing");
    let create_ms = t.elapsed().as_millis();
    let user_before = tree_digest(&fixture.workdir);

    let edits = common::fixture_replace_edits(50);
    let t = Instant::now();
    handle.apply(&edits).expect("apply");
    let apply_ms = t.elapsed().as_millis();

    let t = Instant::now();
    handle.discard();
    let discard_ms = t.elapsed().as_millis();

    assert_eq!(
        tree_digest(&fixture.workdir),
        user_before,
        "user tree intact"
    );
    println!(
        "1k files (gix committed-tree backing): create={}ms apply50={}ms discard={}ms",
        create_ms, apply_ms, discard_ms
    );
}
