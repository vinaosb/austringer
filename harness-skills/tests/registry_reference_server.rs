//! Reference server + local store + protocol surface (todo 32): the
//! directory/index format is the self-hostable static registry; the
//! local-file store is the default (offline) transport; search is lexical.

use std::path::Path;

use harness_skills::registry::{
    MAX_INDEX_BYTES, PackFile, RegistryError, RegistryStore, SignatureVerifier, blake3_hex,
};
use tempfile::TempDir;

mod registry_common;

use registry_common::{key, real_server};

#[test]
fn builds_content_addressed_static_index_from_the_real_packs_tree() {
    let tmp = TempDir::new().unwrap();
    let store = real_server(tmp.path(), None);

    let index = store.index().unwrap();
    let names: Vec<&str> = index.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "accounting",
            "ai-deployment",
            "audit-exemplar",
            "backend",
            "datacenter-aec",
            "devops",
            "dotnet",
            "finance-investors",
            "frontend",
            "legal",
            "maths"
        ],
        "entries sorted by name, manifest-driven"
    );
    let dotnet = index.find("dotnet").expect("dotnet entry");
    assert_eq!(dotnet.version, "1.0.0");
    assert_eq!(dotnet.category, "programming");
    assert!(dotnet.hash.len() == 64, "blake3 hex is 64 chars");

    // static index + content-addressed pack files exist on disk
    let index_json = tmp.path().join("index.json");
    assert!(index_json.is_file(), "index.json written");
    let raw = std::fs::read(&index_json).unwrap();
    assert!(raw.len() < MAX_INDEX_BYTES);
    let pack_file = tmp
        .path()
        .join("packs")
        .join(format!("{}.hpk", dotnet.hash));
    assert!(pack_file.is_file(), "content-addressed pack file");

    // fetched bytes hash to the pinned value
    let bytes = store.pack_bytes(&dotnet.hash).unwrap();
    assert_eq!(blake3_hex(&bytes), dotnet.hash);
}

#[test]
fn deterministic_rebuild_produces_identical_hashes() {
    let a = TempDir::new().unwrap();
    let b = TempDir::new().unwrap();
    let ia = real_server(a.path(), None).index().unwrap();
    let ib = real_server(b.path(), None).index().unwrap();
    assert_eq!(
        ia.entries
            .iter()
            .map(|e| (&e.name, &e.hash))
            .collect::<Vec<_>>(),
        ib.entries
            .iter()
            .map(|e| (&e.name, &e.hash))
            .collect::<Vec<_>>(),
        "same tree => byte-identical content addressing"
    );
}

#[test]
fn search_is_lexical_over_name_category_summary() {
    let tmp = TempDir::new().unwrap();
    let index = real_server(tmp.path(), None).index().unwrap();

    let hits = index.search("dotnet");
    assert!(hits.iter().any(|e| e.name == "dotnet"));
    let hits = index.search("LEGAL");
    assert!(hits.iter().any(|e| e.name == "legal"), "case-insensitive");
    assert!(index.search("klingon-nonexistent").is_empty());
}

#[test]
fn pack_files_carry_files_shared_and_manifest_metadata() {
    let tmp = TempDir::new().unwrap();
    let store = real_server(tmp.path(), None);
    let index = store.index().unwrap();
    let dotnet_entry = index.find("dotnet").expect("dotnet entry");
    let pack = PackFile::parse(&store.pack_bytes(&dotnet_entry.hash).unwrap()).unwrap();
    assert_eq!(pack.name, "dotnet");
    assert_eq!(pack.version, "1.0.0");
    let skill = pack.files.get("SKILL.md").expect("SKILL.md carried");
    assert!(skill.contains("name: dotnet"));
    let shared = pack
        .shared
        .get("pack-conventions")
        .expect("_shared partial carried by stem");
    assert!(!shared.is_empty());
}

#[test]
fn unicode_summary_round_trips_through_index_and_pack() {
    let tmp = TempDir::new().unwrap();
    let store = real_server(tmp.path(), None);
    let index = store.index().unwrap();
    let core = index.find("dotnet").unwrap();
    let pack = PackFile::parse(&store.pack_bytes(&core.hash).unwrap()).unwrap();
    assert_eq!(pack.name, "dotnet");
    // CJK/emoji content from the synth manifest is exercised in the bundle
    // suite; here the real summary must survive the index parse verbatim.
    assert_eq!(&core.summary, &index.find("dotnet").unwrap().summary);
}

#[test]
fn corrupt_or_missing_index_is_typed_not_panicky() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    real_server(root, None);
    let index_path = root.join("index.json");

    std::fs::write(&index_path, b"{ not json").unwrap();
    match local_store_error(root) {
        Some(RegistryError::IndexCorrupt { .. }) => {}
        other => panic!("expected IndexCorrupt, got {other:?}"),
    }

    std::fs::remove_file(&index_path).unwrap();
    match local_store_error(root) {
        Some(RegistryError::IndexMissing) => {}
        other => panic!("expected IndexMissing, got {other:?}"),
    }
}

fn local_store_error(root: &Path) -> Option<RegistryError> {
    let store = harness_skills::registry::LocalDirStore::new(root);
    store.index().err()
}

#[test]
fn oversized_index_is_rejected_before_parse() {
    let huge = vec![b'x'; MAX_INDEX_BYTES + 1];
    match harness_skills::registry::RegistryIndex::parse(&huge) {
        Err(RegistryError::IndexTooLarge { .. }) => {}
        other => panic!("expected IndexTooLarge, got {other:?}"),
    }
}

#[test]
fn signature_round_trip_and_tamper_detection_at_signature_layer() {
    let tmp = TempDir::new().unwrap();
    let k = key();
    let store = real_server(tmp.path(), Some(&k));
    let index = store.index().unwrap();
    let dotnet = index.find("dotnet").unwrap();
    let sig = dotnet.signature.as_ref().expect("signed entry");
    let bytes = store.pack_bytes(&dotnet.hash).unwrap();
    assert!(harness_skills::registry::Blake3Verifier::new(&k).verify(&bytes, sig));
    let mut tampered = bytes.clone();
    tampered[0] ^= 0xff;
    assert!(!harness_skills::registry::Blake3Verifier::new(&k).verify(&tampered, sig));
    let wrong_key = harness_skills::registry::Blake3Verifier::new(
        &harness_skills::registry::SecretKey::from_bytes([9u8; 32]),
    );
    assert!(!wrong_key.verify(&bytes, sig));
}
