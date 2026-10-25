//! Generalized media content blocks (plan M3): widened `MimeType`, the
//! additive `ContentBlock::Media` variant, construction-time meta bounds,
//! the deterministic media marker, and the re-established image-only
//! invariant at the `ImageBlock` boundary.

mod common;

use std::collections::BTreeMap;
use std::io;

use austringer_core::event::content::{
    ContentBlock, ImageBlock, InvalidMediaMeta, InvalidMimeType, MediaBlock, MimeType,
};
use austringer_core::event::{ContentHash, Event, MediaRef, MediaStore};
use common::TempSession;

/// Deterministic fixture bytes whose blake3 short ref is pinned below.
const FIXTURE_BYTES: &[u8] = b"austringer m3 media fixture";

/// `short_ref(FIXTURE_BYTES)` — frozen so the marker snapshot below is an
/// independent pin, not derived from the code under test.
fn fixture_media_ref() -> MediaRef {
    MediaRef {
        hash: ContentHash::from_bytes(FIXTURE_BYTES),
        bytes: FIXTURE_BYTES.len() as u64,
    }
}

fn meta(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn mime_type_accepts_any_major_subtype_when_widened() {
    assert_eq!(
        MimeType::new("audio/wav").map(|mime| mime.as_str().to_owned()),
        Ok("audio/wav".to_owned())
    );
    assert_eq!(
        MimeType::new("model/gltf+json").map(|mime| mime.as_str().to_owned()),
        Ok("model/gltf+json".to_owned())
    );
    assert_eq!(
        MimeType::new("application/pdf").map(|mime| mime.as_str().to_owned()),
        Ok("application/pdf".to_owned())
    );
    // Widening is canonicalizing: uppercase input still lowercases.
    assert_eq!(
        MimeType::new("AUDIO/WAV").map(|mime| mime.as_str().to_owned()),
        Ok("audio/wav".to_owned())
    );
    // The v0.1 image forms are unchanged.
    assert_eq!(MimeType::new("image/png"), Ok(MimeType::png()));
    assert_eq!(MimeType::new("IMAGE/PNG"), Ok(MimeType::png()));
}

#[test]
fn mime_type_rejects_structural_garbage_typed_when_widened() {
    // `not-a-mime` (no slash) was rejected in v0.1 and stays rejected —
    // now through the renamed structural arm.
    assert!(matches!(
        MimeType::new("not-a-mime"),
        Err(InvalidMimeType::InvalidStructure(_))
    ));
    // Missing subtype.
    assert!(matches!(
        MimeType::new("image/"),
        Err(InvalidMimeType::InvalidStructure(_))
    ));
    // Missing major.
    assert!(matches!(
        MimeType::new("/wav"),
        Err(InvalidMimeType::InvalidStructure(_))
    ));
    // More than one slash.
    assert!(matches!(
        MimeType::new("audio/wav/extra"),
        Err(InvalidMimeType::InvalidStructure(_))
    ));
    assert_eq!(MimeType::new(""), Err(InvalidMimeType::Empty));
    assert!(matches!(
        MimeType::new("image/p ng"),
        Err(InvalidMimeType::InvalidCharacters(_))
    ));
}

#[test]
fn mime_type_garbage_loop_never_panics_when_adversarial_inputs_fed() {
    let garbage = [
        "",
        " ",
        "/",
        "//",
        "a/",
        "/b",
        "a/b/c",
        "a/b c",
        "A B/C",
        "image//png",
        "image/png/",
        "/image/png",
        ";;;",
        "привет/мир",
        "image\tpng",
        "image/png\n",
    ];
    for raw in garbage {
        // Every input must resolve to a typed error (or a canonical valid
        // type); none may panic.
        if let Ok(mime) = MimeType::new(raw) {
            assert_eq!(mime.as_str(), mime.as_str().to_ascii_lowercase());
        }
    }
}

#[test]
fn mime_type_image_constructor_enforces_image_major() {
    assert_eq!(
        MimeType::image("image/png").map(|mime| mime.as_str().to_owned()),
        Ok("image/png".to_owned())
    );
    assert_eq!(
        MimeType::image("IMAGE/WEBP").map(|mime| mime.as_str().to_owned()),
        Ok("image/webp".to_owned())
    );
    let error = MimeType::image("audio/wav").expect_err("non-image major must fail");
    assert!(
        matches!(error, InvalidMimeType::NotImage(ref raw) if raw == "audio/wav"),
        "wrong error: {error:?}"
    );
    // Structural garbage fails the same way as `new`.
    assert!(matches!(
        MimeType::image("not-a-mime"),
        Err(InvalidMimeType::InvalidStructure(_))
    ));
    assert_eq!(MimeType::image(""), Err(InvalidMimeType::Empty));
}

#[test]
fn image_block_new_rejects_non_image_mime_when_pinned() {
    let media = fixture_media_ref();
    let image = ImageBlock::new("image/png", 800, 600, media).expect("image mime accepted");
    assert_eq!(image.mime, MimeType::png());
    assert_eq!(image.width, 800);
    assert_eq!(image.height, 600);
    assert_eq!(image.media, media);
    // PINNED: a widened mime must never land inside an ImageBlock.
    let error = ImageBlock::new("audio/wav", 1, 1, media)
        .expect_err("audio mime into ImageBlock must fail");
    assert!(
        matches!(error, InvalidMimeType::NotImage(ref raw) if raw == "audio/wav"),
        "wrong error: {error:?}"
    );
}

#[test]
fn media_block_constructs_when_meta_within_bounds() {
    let block = MediaBlock::new(
        MimeType::png(),
        fixture_media_ref(),
        meta(&[("format", "png")]),
    )
    .expect("valid meta accepted");
    assert_eq!(block.mime, MimeType::png());
    assert_eq!(block.media, fixture_media_ref());
    assert_eq!(block.meta.get("format").map(String::as_str), Some("png"));
    // Boundary values are inclusive: exactly 16 entries, exactly 128 bytes.
    let mut full = BTreeMap::new();
    for index in 0..16 {
        full.insert(format!("k{index:02}"), "v".to_owned());
    }
    assert!(MediaBlock::new(MimeType::png(), fixture_media_ref(), full).is_ok());
    let edge_key = "k".repeat(128);
    let edge_value = "v".repeat(128);
    assert!(
        MediaBlock::new(
            MimeType::png(),
            fixture_media_ref(),
            meta(&[(&edge_key, &edge_value)])
        )
        .is_ok()
    );
}

#[test]
fn media_block_rejects_oversized_meta_typed_at_construction() {
    let mut too_many = BTreeMap::new();
    for index in 0..17 {
        too_many.insert(format!("k{index:02}"), "v".to_owned());
    }
    assert_eq!(
        MediaBlock::new(MimeType::png(), fixture_media_ref(), too_many),
        Err(InvalidMediaMeta::TooManyEntries(17))
    );
    let long_key = "k".repeat(129);
    assert_eq!(
        MediaBlock::new(
            MimeType::png(),
            fixture_media_ref(),
            meta(&[(&long_key, "v")])
        ),
        Err(InvalidMediaMeta::KeyTooLong(long_key.clone()))
    );
    let long_value = "v".repeat(129);
    assert_eq!(
        MediaBlock::new(
            MimeType::png(),
            fixture_media_ref(),
            meta(&[("k", &long_value)])
        ),
        Err(InvalidMediaMeta::ValueTooLong(long_value.clone()))
    );
}

#[test]
fn media_block_marker_is_deterministic_snapshot() {
    let block = MediaBlock::new(
        MimeType::new("audio/wav").expect("widened mime"),
        fixture_media_ref(),
        meta(&[("format", "wav")]),
    )
    .expect("valid block");
    // Inline snapshot (independent pin): short_ref of FIXTURE_BYTES is
    // blake3:81661a02cc5db53b… recorded once and frozen.
    assert_eq!(
        block.text_marker(),
        "[media: audio/wav blake3:81661a02cc5db53b… format=wav]"
    );
    assert_eq!(block.text_marker(), block.text_marker());
    // No-meta form: the marker closes right after the short hash.
    let bare = MediaBlock::new(
        MimeType::new("model/gltf+json").expect("widened"),
        fixture_media_ref(),
        BTreeMap::new(),
    )
    .expect("valid block");
    assert_eq!(
        bare.text_marker(),
        "[media: model/gltf+json blake3:81661a02cc5db53b…]"
    );
}

#[test]
fn media_block_marker_lists_meta_keys_sorted() {
    // Insertion order is scrambled; BTreeMap iteration (and the marker)
    // must be sorted.
    let block = MediaBlock::new(
        MimeType::new("application/pdf").expect("widened"),
        fixture_media_ref(),
        meta(&[("zz", "last"), ("aa", "first"), ("mm", "mid")]),
    )
    .expect("valid block");
    let marker = block.text_marker();
    let aa = marker.find("aa=first").expect("aa entry");
    let mm = marker.find("mm=mid").expect("mm entry");
    let zz = marker.find("zz=last").expect("zz entry");
    assert!(aa < mm && mm < zz, "marker keys not sorted: {marker}");
    assert_eq!(
        marker,
        "[media: application/pdf blake3:81661a02cc5db53b… aa=first mm=mid zz=last]"
    );
}

#[test]
fn media_content_block_roundtrips_through_serde_when_in_event() {
    let block = MediaBlock::new(
        MimeType::new("audio/wav").expect("widened"),
        fixture_media_ref(),
        meta(&[("format", "wav")]),
    )
    .expect("valid block");
    let event = Event::User {
        blocks: vec![ContentBlock::Media(block)],
    };
    let encoded = serde_json::to_string(&event).expect("serialize");
    assert!(
        encoded.contains(r#""type":"media""#),
        "additive variant must tag as `media`: {encoded}"
    );
    // Meta keys serialize sorted (BTreeMap), never insertion-ordered.
    let scrambled = Event::User {
        blocks: vec![ContentBlock::Media(
            MediaBlock::new(
                MimeType::new("application/pdf").expect("widened"),
                fixture_media_ref(),
                meta(&[("zz", "1"), ("aa", "2")]),
            )
            .expect("valid block"),
        )],
    };
    let encoded_scrambled = serde_json::to_string(&scrambled).expect("serialize");
    let aa = encoded_scrambled.find("\"aa\"").expect("aa key");
    let zz = encoded_scrambled.find("\"zz\"").expect("zz key");
    assert!(
        aa < zz,
        "meta keys must serialize sorted: {encoded_scrambled}"
    );
    // Round-trip equality and double-serialization byte stability.
    let decoded: Event = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(decoded, event);
    assert_eq!(
        serde_json::to_string(&decoded).expect("re-serialize"),
        encoded
    );
    // The block-level marker is reachable through the enum too.
    let Event::User { blocks } = &decoded else {
        panic!("round-trip must decode to a user event");
    };
    assert_eq!(
        blocks[0].text_marker(),
        "[media: audio/wav blake3:81661a02cc5db53b… format=wav]"
    );
}

#[test]
fn media_store_rejects_non_image_mime_typed() {
    let session = TempSession::new("media-blocks-store");
    let store = MediaStore::open(session.path()).expect("open media store");
    let audio = MimeType::new("audio/wav").expect("widened mime");
    let error = store
        .store(&audio, 1, 1, FIXTURE_BYTES)
        .expect_err("image store must refuse non-image mime");
    assert_eq!(
        error.kind(),
        io::ErrorKind::InvalidInput,
        "store must fail typed InvalidInput, got: {error}"
    );
    assert!(
        error.to_string().contains("not an image"),
        "error must name the invariant: {error}"
    );
    // The image path keeps working.
    store
        .store(&MimeType::png(), 2, 2, FIXTURE_BYTES)
        .expect("image mime still storable");
}
