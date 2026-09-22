//! Media-store tests: out-of-line image storage under `<session>/media/`,
//! content-hash references, dedup, and the never-inline-base64 invariant of
//! the serialized event form.

mod common;

use std::fs;

use common::TempSession;
use harness_core::event::content::{ContentBlock, MimeType};
use harness_core::event::{Event, MediaStore, RecordedEvent, Sequence};

fn deterministic_bytes(seed: u64, len: usize) -> Vec<u8> {
    // xorshift64* re-implemented locally; the common helper owns the struct.
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D) as u8
        })
        .collect()
}

#[test]
fn store_persists_bytes_when_image_stored() {
    let session = TempSession::new("media-store");
    let store = MediaStore::open(session.path()).expect("open media store");
    let bytes = deterministic_bytes(11, 1024);

    let block = store
        .store(&MimeType::png(), 800, 600, &bytes)
        .expect("store image");

    assert_eq!(block.width, 800);
    assert_eq!(block.height, 600);
    assert_eq!(block.mime, MimeType::png());
    assert_eq!(block.media.bytes, 1024);
    let loaded = store
        .load(&block.media.hash)
        .expect("load")
        .expect("present");
    assert_eq!(loaded, bytes, "stored bytes must round-trip");
}

#[test]
fn store_dedupes_when_identical_content_stored_twice() {
    let session = TempSession::new("media-dedup");
    let store = MediaStore::open(session.path()).expect("open media store");
    let bytes = deterministic_bytes(12, 512);

    let first = store
        .store(&MimeType::png(), 10, 10, &bytes)
        .expect("first store");
    let second = store
        .store(&MimeType::png(), 10, 10, &bytes)
        .expect("second store");
    assert_eq!(first.media.hash, second.media.hash);

    let media_dir = session.path().join("media");
    let entries: Vec<_> = fs::read_dir(&media_dir)
        .expect("media dir")
        .collect::<Result<_, _>>()
        .expect("entries");
    assert_eq!(
        entries.len(),
        1,
        "identical content must not duplicate files"
    );
}

#[test]
fn load_returns_none_when_hash_unknown() {
    let session = TempSession::new("media-unknown");
    let store = MediaStore::open(session.path()).expect("open media store");
    let absent = store
        .store(&MimeType::png(), 1, 1, &deterministic_bytes(13, 32))
        .expect("store");
    let bytes = deterministic_bytes(999, 32); // different content, never stored
    let unknown = harness_core::event::ContentHash::from_bytes(&bytes);
    assert_ne!(unknown, absent.media.hash);
    assert!(store.load(&unknown).expect("load").is_none());
}

#[test]
fn jsonl_never_inlines_image_bytes_when_event_serialized() {
    let session = TempSession::new("media-no-inline");
    let store = MediaStore::open(session.path()).expect("open media store");
    let bytes = deterministic_bytes(14, 4096); // 4 KiB would bloat any inline form
    let block = store
        .store(&MimeType::png(), 640, 480, &bytes)
        .expect("store image");

    let recorded = RecordedEvent {
        seq: Sequence(1),
        event: Event::User {
            blocks: vec![ContentBlock::Image(block)],
        },
    };
    let line = serde_json::to_string(&recorded).expect("serialize");

    assert!(
        line.contains("blake3:"),
        "line must reference media by hash"
    );
    assert!(
        line.len() < 600,
        "4 KiB image must not be inlined (base64 would exceed 5400 chars): {} bytes",
        line.len()
    );
}

#[test]
fn store_media_roundtrips_arbitrary_bytes_when_loaded_by_hash() {
    let session = TempSession::new("media-store-general");
    let store = MediaStore::open(session.path()).expect("open media store");
    let bytes = b"%PDF-1.7\narbitrary non-image payload".to_vec();

    let media = store.store_media(&bytes).expect("store media");

    assert_eq!(media.bytes, bytes.len() as u64);
    assert_eq!(
        store.load(&media.hash).expect("load").expect("present"),
        bytes,
        "generalized store must round-trip any payload"
    );
    let media_dir = session.path().join("media");
    let entries: Vec<_> = fs::read_dir(&media_dir)
        .expect("media dir")
        .collect::<Result<_, _>>()
        .expect("entries");
    assert_eq!(entries.len(), 1, "one content-addressed file, no dimensions");
}

#[test]
fn store_media_dedupes_when_identical_bytes_stored_twice() {
    let session = TempSession::new("media-store-general-dedup");
    let store = MediaStore::open(session.path()).expect("open media store");
    let bytes = deterministic_bytes(21, 256);

    let first = store.store_media(&bytes).expect("first store");
    let second = store.store_media(&bytes).expect("second store");
    assert_eq!(first, second);

    let media_dir = session.path().join("media");
    let entries: Vec<_> = fs::read_dir(&media_dir)
        .expect("media dir")
        .collect::<Result<_, _>>()
        .expect("entries");
    assert_eq!(
        entries.len(),
        1,
        "identical content must not duplicate files"
    );
}

#[test]
fn text_marker_is_deterministic_when_image_rendered_for_text_consumers() {
    let session = TempSession::new("media-marker");
    let store = MediaStore::open(session.path()).expect("open media store");
    let block = store
        .store(&MimeType::jpeg(), 1920, 1080, &deterministic_bytes(15, 64))
        .expect("store image");

    let first = block.text_marker();
    let second = block.text_marker();
    assert_eq!(first, second, "marker must be deterministic");
    assert!(
        first.contains("image/jpeg") && first.contains("1920x1080"),
        "marker must carry mime and dimensions: {first}"
    );
    assert!(
        first.contains("blake3:"),
        "marker must identify content: {first}"
    );
}
