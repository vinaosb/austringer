//! Derived-store tests: projection artifacts plus provenance manifests
//! under `<session>/media/derived/`, cache semantics (miss, dedup,
//! overwrite), typed corruption failures for every tamper variant, and
//! crash-safety of the pair write (manifest last).

mod common;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use austringer_core::event::{ContentHash, DerivedCorrupt, DerivedStore, ProjectionKind};
use common::TempSession;

const PRODUCER: &str = "whisper-cpp";

fn source_hash(seed: u8) -> ContentHash {
    ContentHash::from_bytes(&[seed; 32])
}

fn derived_dirs(session: &TempSession) -> (PathBuf, PathBuf) {
    let root = session.path().join("media").join("derived");
    (root.join("artifacts"), root.join("manifests"))
}

fn sole_file(dir: &Path) -> PathBuf {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read dir")
        .map(|entry| entry.expect("entry").path())
        .collect();
    files.sort();
    assert_eq!(files.len(), 1, "expected exactly one file in {dir:?}");
    files.remove(0)
}

#[test]
fn store_derived_roundtrips_when_projection_cached() {
    let session = TempSession::new("derived-roundtrip");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(1);
    let bytes = b"transcript: hello austringer".to_vec();

    let artifact = store
        .store_derived(&source, PRODUCER, ProjectionKind::Transcript, &bytes)
        .expect("store derived");

    assert_eq!(artifact.bytes, bytes.len() as u64);
    let loaded = store
        .load_derived(&source, PRODUCER, ProjectionKind::Transcript)
        .expect("load derived")
        .expect("cached entry");
    assert_eq!(loaded, bytes);

    let (artifacts, manifests) = derived_dirs(&session);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(sole_file(&manifests)).expect("manifest"))
            .expect("manifest json");
    assert_eq!(manifest["source"], serde_json::json!(source.to_ref()));
    assert_eq!(manifest["producer"], serde_json::json!(PRODUCER));
    assert_eq!(manifest["kind"], serde_json::json!("transcript"));
    assert!(manifest["created_ms"].is_u64());
    assert_eq!(
        manifest["artifact"]["hash"],
        serde_json::json!(artifact.hash.to_ref())
    );
    assert_eq!(
        manifest["artifact"]["bytes"],
        serde_json::json!(bytes.len())
    );
    assert_eq!(
        fs::read(sole_file(&artifacts)).expect("artifact file"),
        bytes,
        "artifact file is the content-addressed projection itself"
    );
    let session_entries: Vec<String> = fs::read_dir(session.path())
        .expect("session dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        session_entries,
        vec!["media".to_string()],
        "projections stay under media/derived/, never in the session JSONL"
    );
}

#[test]
fn load_derived_returns_none_when_key_never_stored() {
    let session = TempSession::new("derived-miss");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    assert!(
        store
            .load_derived(&source_hash(2), PRODUCER, ProjectionKind::Thumbnail)
            .expect("lookup")
            .is_none(),
        "never-stored key is a plain cache miss"
    );
}

#[test]
fn store_derived_dedupes_when_same_key_and_bytes_restored() {
    let session = TempSession::new("derived-dedup");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(3);
    let bytes = b"projection text".to_vec();

    store
        .store_derived(&source, PRODUCER, ProjectionKind::TextProjection, &bytes)
        .expect("first store");
    let (artifacts, manifests) = derived_dirs(&session);
    let manifest_before = fs::read(sole_file(&manifests)).expect("manifest before");

    let second = store
        .store_derived(&source, PRODUCER, ProjectionKind::TextProjection, &bytes)
        .expect("second store");

    assert_eq!(second.hash, ContentHash::from_bytes(&bytes));
    assert_eq!(
        fs::read(sole_file(&manifests)).expect("manifest after"),
        manifest_before,
        "identical re-store must write nothing (created_ms stays stable)"
    );
    assert_eq!(
        fs::read_dir(&artifacts).expect("artifacts dir").count(),
        1,
        "identical content must not duplicate artifacts"
    );
}

#[test]
fn store_derived_overwrites_entry_when_same_key_restored_with_new_bytes() {
    let session = TempSession::new("derived-overwrite");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(4);
    let kind = ProjectionKind::TextProjection;

    let first = store
        .store_derived(&source, PRODUCER, kind, b"projection v1")
        .expect("first store");
    let second = store
        .store_derived(&source, PRODUCER, kind, b"projection v2 (regenerated)")
        .expect("second store");
    assert_ne!(first.hash, second.hash);

    let loaded = store
        .load_derived(&source, PRODUCER, kind)
        .expect("load")
        .expect("cached");
    assert_eq!(loaded, b"projection v2 (regenerated)");

    let (artifacts, manifests) = derived_dirs(&session);
    assert_eq!(
        fs::read_dir(&artifacts).expect("artifacts dir").count(),
        2,
        "superseded artifact is deliberately kept: no garbage collection"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(sole_file(&manifests)).expect("manifest"))
            .expect("manifest json");
    assert_eq!(
        manifest["artifact"]["hash"],
        serde_json::json!(second.hash.to_ref()),
        "manifest points at the newest artifact"
    );
}

#[test]
fn crash_between_pair_writes_leaves_ignorable_orphan_when_manifest_missing() {
    let session = TempSession::new("derived-crash-window");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(5);

    // Complete store, then drop the manifest: the remaining artifact is
    // exactly what a crash after the artifact write (before the manifest
    // write) leaves behind.
    store
        .store_derived(
            &source,
            PRODUCER,
            ProjectionKind::SceneDescription,
            b"scene",
        )
        .expect("store");
    let (artifacts, manifests) = derived_dirs(&session);
    fs::remove_file(sole_file(&manifests)).expect("drop manifest");
    assert!(
        store
            .load_derived(&source, PRODUCER, ProjectionKind::SceneDescription)
            .expect("lookup after crash")
            .is_none(),
        "orphan artifact without manifest is an ignorable cache miss"
    );
    assert_eq!(
        fs::read_dir(&artifacts).expect("artifacts dir").count(),
        1,
        "orphan artifact is kept (no GC)"
    );

    // A foreign file in the artifact dir (never referenced by any
    // manifest) is equally ignorable.
    fs::write(artifacts.join("foreign-orphan"), b"junk").expect("foreign orphan");
    assert!(
        store
            .load_derived(&source, PRODUCER, ProjectionKind::SceneDescription)
            .expect("lookup with foreign orphan")
            .is_none()
    );
}

#[test]
fn load_derived_fails_typed_when_manifest_truncated() {
    let session = TempSession::new("derived-truncated");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(6);
    let kind = ProjectionKind::Transcript;
    store
        .store_derived(&source, PRODUCER, kind, b"words")
        .expect("store");
    let (_, manifests) = derived_dirs(&session);
    let path = sole_file(&manifests);
    let raw = fs::read(&path).expect("manifest");
    fs::write(&path, &raw[..raw.len() / 2]).expect("truncate manifest");

    let error = store
        .load_derived(&source, PRODUCER, kind)
        .expect_err("truncated manifest must fail typed");
    assert!(
        matches!(&error, DerivedCorrupt::Malformed { producer, .. } if producer == PRODUCER),
        "expected Malformed, got {error:?}"
    );
}

#[test]
fn load_derived_fails_typed_when_manifest_source_hash_swapped() {
    let session = TempSession::new("derived-source-swap");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(7);
    let kind = ProjectionKind::Transcript;
    store
        .store_derived(&source, PRODUCER, kind, b"words")
        .expect("store");
    let (_, manifests) = derived_dirs(&session);
    let path = sole_file(&manifests);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("manifest")).expect("json");
    manifest["source"] = serde_json::json!(source_hash(99).to_ref());
    fs::write(&path, serde_json::to_vec(&manifest).expect("re-encode")).expect("tamper");

    let error = store
        .load_derived(&source, PRODUCER, kind)
        .expect_err("tampered source must fail typed");
    assert!(
        matches!(&error, DerivedCorrupt::KeyMismatch { requested, .. } if *requested == source),
        "expected KeyMismatch, got {error:?}"
    );
}

#[test]
fn load_derived_fails_typed_when_artifact_swapped() {
    let session = TempSession::new("derived-artifact-swap");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(8);
    let kind = ProjectionKind::Transcript;
    let artifact = store
        .store_derived(&source, PRODUCER, kind, b"original words")
        .expect("store");
    let (artifacts, manifests) = derived_dirs(&session);
    let listing = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    };
    eprintln!(
        "BEFORE TAMPER artifacts={:?} manifests={:?}",
        listing(&artifacts),
        listing(&manifests)
    );
    let replacement = b"swapped bytes";
    fs::write(sole_file(&artifacts), replacement).expect("swap artifact");
    eprintln!(
        "AFTER TAMPER  artifacts={:?} manifests={:?}",
        listing(&artifacts),
        listing(&manifests)
    );

    let error = store
        .load_derived(&source, PRODUCER, kind)
        .expect_err("swapped artifact must fail typed");
    assert!(
        matches!(
            &error,
            DerivedCorrupt::ArtifactHash { stored, actual }
                if *stored == artifact.hash && *actual == ContentHash::from_bytes(replacement)
        ),
        "expected ArtifactHash with both hashes, got {error:?}"
    );
}

#[test]
fn load_derived_fails_typed_when_manifest_references_missing_artifact() {
    let session = TempSession::new("derived-missing-artifact");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(9);
    let kind = ProjectionKind::Transcript;
    let artifact = store
        .store_derived(&source, PRODUCER, kind, b"words")
        .expect("store");
    let (artifacts, _) = derived_dirs(&session);
    fs::remove_file(sole_file(&artifacts)).expect("drop artifact");

    let error = store
        .load_derived(&source, PRODUCER, kind)
        .expect_err("dangling manifest must fail typed");
    assert!(
        matches!(&error, DerivedCorrupt::MissingArtifact { stored } if *stored == artifact.hash),
        "expected MissingArtifact, got {error:?}"
    );
}

#[test]
fn store_derived_rejects_empty_and_oversized_producer_typed() {
    let session = TempSession::new("derived-producer-bounds");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(10);
    let kind = ProjectionKind::Thumbnail;

    for producer in ["", &"p".repeat(129)] {
        let error = store
            .store_derived(&source, producer, kind, b"x")
            .expect_err("invalid producer must be rejected typed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
    store
        .store_derived(&source, &"p".repeat(128), kind, b"x")
        .expect("128-byte producer is accepted at the boundary");
}

#[test]
fn distinct_producers_stay_distinct_when_slugs_sanitize_identically() {
    let session = TempSession::new("derived-slug-collision");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(11);
    let kind = ProjectionKind::Transcript;

    store
        .store_derived(&source, "whisper:cpp", kind, b"A")
        .expect("store colon producer");
    store
        .store_derived(&source, "whisper-cpp", kind, b"B")
        .expect("store dash producer");

    let (_, manifests) = derived_dirs(&session);
    assert_eq!(
        fs::read_dir(&manifests).expect("manifests dir").count(),
        2,
        "distinct producers must never share a manifest file"
    );
    assert_eq!(
        store
            .load_derived(&source, "whisper:cpp", kind)
            .expect("load colon")
            .expect("cached"),
        b"A"
    );
    assert_eq!(
        store
            .load_derived(&source, "whisper-cpp", kind)
            .expect("load dash")
            .expect("cached"),
        b"B"
    );
}

#[test]
fn projection_kinds_are_distinct_cache_entries_for_same_producer() {
    let session = TempSession::new("derived-kind-distinct");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(12);

    store
        .store_derived(
            &source,
            PRODUCER,
            ProjectionKind::TextProjection,
            b"structure",
        )
        .expect("store text");
    store
        .store_derived(&source, PRODUCER, ProjectionKind::Transcript, b"words")
        .expect("store transcript");

    assert_eq!(
        store
            .load_derived(&source, PRODUCER, ProjectionKind::TextProjection)
            .expect("load text")
            .expect("cached"),
        b"structure"
    );
    assert_eq!(
        store
            .load_derived(&source, PRODUCER, ProjectionKind::Transcript)
            .expect("load transcript")
            .expect("cached"),
        b"words"
    );
    assert!(
        store
            .load_derived(&source, PRODUCER, ProjectionKind::Thumbnail)
            .expect("load thumbnail")
            .is_none()
    );
}

#[test]
fn hostile_producer_ids_roundtrip_typed_without_panicking() {
    let session = TempSession::new("derived-hostile-producers");
    let store = DerivedStore::open(session.path()).expect("open derived store");
    let source = source_hash(13);
    let hostile = [
        "model:anthropic/claude-sonnet-4-6",
        "a b\tc",
        "..\\..\\evil",
        "../evil",
        "привет-модель",
        "🦀crab",
        "PROD.v2_x",
    ];
    for (index, producer) in hostile.iter().enumerate() {
        let bytes = format!("payload-{index}");
        store
            .store_derived(
                &source,
                producer,
                ProjectionKind::TextProjection,
                bytes.as_bytes(),
            )
            .expect("hostile producer must store");
        assert_eq!(
            store
                .load_derived(&source, producer, ProjectionKind::TextProjection)
                .expect("hostile producer must load")
                .expect("cached"),
            bytes.as_bytes(),
            "producer {producer:?} must round-trip"
        );
    }
    let (_, manifests) = derived_dirs(&session);
    for entry in fs::read_dir(&manifests).expect("manifests dir") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        assert!(
            Path::new(&name).components().count() == 1,
            "manifest name must stay a single path component: {name:?}"
        );
    }
}
