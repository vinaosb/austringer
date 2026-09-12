//! Streaming spill tests (plan todo 19): memory-bounded raw-stream spill
//! into the T6 SpillStore layout, with pointer recovery.

use harness_core::tools::{IoCap, SpillStore};
use harness_exec::spill::RawSpool;

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn under_cap_output_never_spills() {
    let dir = tempdir();
    let store = SpillStore::open(dir.path()).expect("store");
    let mut spool = RawSpool::new(store, IoCap::DEFAULT);
    spool.write(b"cargo 1.98.0\n").expect("write");
    assert_eq!(spool.total_bytes(), 13);
    let spilled = spool.finish().expect("finish");
    assert!(spilled.is_none(), "small output stays in memory only");
}

#[test]
fn over_cap_output_streams_to_disk_and_recovers() {
    let dir = tempdir();
    let store = SpillStore::open(dir.path()).expect("store");
    let cap = IoCap {
        max_inline_bytes: 1024,
    };
    let mut spool = RawSpool::new(store, cap);

    // 256 KiB written in small chunks: memory stays O(cap), disk grows.
    let chunk = vec![b'x'; 4096];
    let rounds = 64;
    for _ in 0..rounds {
        spool.write(&chunk).expect("write");
    }
    assert_eq!(spool.total_bytes(), (rounds * 4096) as u64);

    let spilled = spool.finish().expect("finish").expect("must spill");
    assert_eq!(spilled.raw_bytes, (rounds * 4096) as u64);

    // Pointer recovers the exact stream through the T6 store.
    let store = SpillStore::open(dir.path()).expect("store reopen");
    let recovered = store.read(&spilled.hash).expect("read").expect("present");
    assert_eq!(recovered.len(), rounds * 4096);
    assert!(recovered.iter().all(|b| *b == b'x'));
}

#[test]
fn spilled_hash_matches_blake3_of_stream() {
    let dir = tempdir();
    let store = SpillStore::open(dir.path()).expect("store");
    let mut spool = RawSpool::new(
        store,
        IoCap {
            max_inline_bytes: 8,
        },
    );
    let payload: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
    spool.write(&payload).expect("write");
    let spilled = spool.finish().expect("finish").expect("spilled");
    assert_eq!(spilled.hash.to_ref(), blake3_ref(&payload));
}

fn blake3_ref(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

#[test]
fn finish_is_idempotent_about_temp_files() {
    let dir = tempdir();
    let store = SpillStore::open(dir.path()).expect("store");
    let mut spool = RawSpool::new(
        store,
        IoCap {
            max_inline_bytes: 16,
        },
    );
    spool.write(&[b'a'; 1024]).expect("write");
    let spilled = spool.finish().expect("finish").expect("spilled");
    let store_dir = dir.path().join("spill");
    let entries: Vec<_> = std::fs::read_dir(&store_dir)
        .expect("dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // Exactly the one content-addressed file; no temp litter.
    assert_eq!(
        entries,
        vec![spilled.hash.to_ref().trim_start_matches("blake3:")]
    );
}
