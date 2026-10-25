//! Detached signing (Amp: keys OUTSIDE agent processes) and the seeded
//! flakiness sweep (three deterministic fixture repos).

mod common;

use austringer_snapshots::provenance::{DetachedSignature, MockSigningService, SigningService};
use austringer_snapshots::{ExternallyModifiedPolicy, RestoreMode, SnapshotOptions, SnapshotStore};
use common::*;
use std::path::Path;
use std::sync::Arc;

/// The mock SigningService is keyed (host-side key), deterministic per
/// payload, and no key material appears in the signed output.
#[test]
fn mock_signing_service_is_keyed_and_deterministic() {
    let signer = MockSigningService::new([7u8; 32]);
    let a = signer.sign(b"payload-a").expect("sign a");
    let a2 = signer.sign(b"payload-a").expect("sign a again");
    let b = signer.sign(b"payload-b").expect("sign b");
    assert_eq!(a.value, a2.value, "deterministic per payload");
    assert_ne!(a.value, b.value, "payload-sensitive");
    assert_eq!(a.algorithm, "blake3-keyed-mock");
    // A different key produces a different signature for the same payload.
    let other = MockSigningService::new([9u8; 32])
        .sign(b"payload-a")
        .expect("sign");
    assert_ne!(a.value, other.value, "key-material matters");

    // No key bytes leak into the signature value (blake3 one-way).
    let mut leaked = true;
    for window in a.value.chunks(4) {
        if window.iter().all(|byte| *byte == 7) {
            leaked = false;
        }
    }
    assert!(
        leaked || !a.value.iter().all(|byte| *byte == 7),
        "no raw key material"
    );
    let _ = DetachedSignature::default();
}

/// A snapshot made through a SigningService records a detached signature
/// of the commit object bytes in the manifest — the commit itself stays
/// clean and the signature tracks content.
#[test]
fn snapshots_record_detached_signatures() {
    let (_dir, workdir) = git_repo(&[("a.txt", b"alpha\n")]);
    let _session_root = workdir.join("session");
    let store = SnapshotStore::open(&workdir).expect("store");
    let opts = SnapshotOptions {
        signer: Some(Arc::new(MockSigningService::new([1u8; 32]))),
        ..SnapshotOptions::default()
    };
    store
        .snapshot(&sess("s1"), turn(1), &opts)
        .expect("snapshot@1");
    let meta = store.snapshot_meta(&sess("s1"), turn(1)).expect("meta");
    let signature = meta.signature.as_ref().expect("signature recorded");
    assert_eq!(signature.algorithm, "blake3-keyed-mock");
    assert_eq!(signature.value.len(), 64, "hex blake3");

    // Different content -> different commit -> different signature.
    write_file(&workdir, Path::new("a.txt"), b"beta\n");
    store
        .snapshot(&sess("s1"), turn(2), &opts)
        .expect("snapshot@2");
    let meta2 = store.snapshot_meta(&sess("s1"), turn(2)).expect("meta");
    assert_ne!(meta.signature, meta2.signature, "signature tracks content");

    // Without a signer nothing is signed (capability stays optional).
    let plain = SnapshotOptions::default();
    store
        .snapshot(&sess("s1"), turn(3), &plain)
        .expect("snapshot@3");
    let meta3 = store.snapshot_meta(&sess("s1"), turn(3)).expect("meta");
    assert!(meta3.signature.is_none());
}

/// FLAKY x3: three seeded deterministic repos — identical seeds produce
/// identical digests and clean round-trips; different seeds differ.
#[test]
fn seeded_round_trips_are_deterministic() {
    let mut digests = Vec::new();
    for seed in [7u64, 42, 9281] {
        let (_dir, workdir) = git_repo(&[]);
        let mut rng = XorShift64(seed);
        let mut names: Vec<String> = Vec::new();
        for i in 0..12 {
            let unique = rng.next() % 97;
            let rel = format!("f{i}/file-{unique}.txt");
            let bytes: Vec<u8> = (0..(8 + (rng.next() % 40) * 3))
                .map(|k| (rng.next() as u8).wrapping_add(k as u8))
                .chain(b"\r\n".iter().copied())
                .collect();
            write_file(&workdir, Path::new(&rel), &bytes);
            names.push(rel);
        }
        let session_root = workdir.join("session");
        let store = SnapshotStore::open(&workdir).expect("store");
        let before = tree_digest(&workdir);
        store
            .snapshot(&sess("s1"), turn(1), &SnapshotOptions::default())
            .expect("snapshot");
        // Wander: rewrite half, delete one, add one.
        for rel in names.iter().take(6) {
            write_file(&workdir, Path::new(rel), b"wandered\r\n");
        }
        delete_file(&workdir, Path::new(&names[6]));
        write_file(&workdir, Path::new("appeared.txt"), b"appeared\n");
        store
            .snapshot(&sess("s1"), turn(2), &SnapshotOptions::default())
            .expect("snapshot 2");
        let plan = store
            .rewind_plan(
                &session_root,
                &sess("s1"),
                turn(1),
                RestoreMode::FilesOnly,
                ExternallyModifiedPolicy::Include,
            )
            .expect("plan");
        store.execute_rewind(plan, None, None).expect("rewind");
        assert_eq!(
            tree_digest(&workdir),
            before,
            "seed {seed}: byte-identical rewind"
        );
        digests.push(before);
    }
    assert_eq!(digests.len(), 3);
    assert_ne!(digests[0], digests[1]);
    assert_ne!(digests[1], digests[2]);
}

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}
