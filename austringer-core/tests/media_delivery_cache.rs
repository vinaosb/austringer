//! Delivery-planner cache + adversarial tests (plan M8): cached-projection
//! inlining (right kind only, after the marker, never on native paths),
//! unknown MIME kinds, and the deterministic hash-keyed text rendering of
//! every wire block.

mod common;

use std::collections::BTreeMap;

use austringer_core::event::content::MimeType;
use austringer_core::event::derived::ProjectionKind;
use austringer_core::event::media::ContentHash;
use austringer_core::media_delivery::{WireBlock, plan};
use common::deliverykit::MapCache;
use common::deliverykit::{
    AUDIO_BYTES, IMAGE_BYTES, audio, image_block, image_marker, media_block, media_marker,
    media_of, no_cache, text_only, vision, vision_audio,
};

fn scene_cache(text: &str) -> MapCache {
    MapCache(vec![(
        ContentHash::from_bytes(IMAGE_BYTES),
        ProjectionKind::SceneDescription,
        text.to_owned(),
    )])
}

#[test]
fn cached_projection_inlines_as_text_after_marker() {
    let plan = plan(
        &[image_block()],
        text_only(),
        &scene_cache("a red square on white"),
    );
    assert_eq!(
        plan.wire,
        vec![
            WireBlock::Marker(image_marker()),
            WireBlock::Text("a red square on white".to_owned()),
        ]
    );
    assert_eq!(
        plan.needed_projections,
        vec![(media_of(IMAGE_BYTES), ProjectionKind::SceneDescription)],
        "the queue stays populated even when a cached copy was inlined"
    );
}

#[test]
fn cached_transcript_inlines_for_incapable_audio() {
    let cache = MapCache(vec![(
        ContentHash::from_bytes(AUDIO_BYTES),
        ProjectionKind::Transcript,
        "speaker one: hello".to_owned(),
    )]);
    let plan = plan(
        &[media_block("audio/wav", AUDIO_BYTES, &[])],
        vision(),
        &cache,
    );
    assert_eq!(
        plan.wire,
        vec![
            WireBlock::Marker(media_marker("audio/wav", AUDIO_BYTES, &[])),
            WireBlock::Text("speaker one: hello".to_owned()),
        ]
    );
}

#[test]
fn cached_projection_of_wrong_kind_is_not_inlined() {
    let cache = MapCache(vec![(
        ContentHash::from_bytes(IMAGE_BYTES),
        ProjectionKind::Thumbnail,
        "ignored".to_owned(),
    )]);
    let plan = plan(&[image_block()], text_only(), &cache);
    assert_eq!(plan.wire, vec![WireBlock::Marker(image_marker())]);
}

#[test]
fn native_paths_never_inline_or_queue() {
    let cache = MapCache(vec![
        (
            ContentHash::from_bytes(IMAGE_BYTES),
            ProjectionKind::SceneDescription,
            "unused for capable models".to_owned(),
        ),
        (
            ContentHash::from_bytes(AUDIO_BYTES),
            ProjectionKind::Transcript,
            "unused for capable models".to_owned(),
        ),
    ]);
    let plan = plan(
        &[image_block(), media_block("audio/wav", AUDIO_BYTES, &[])],
        vision_audio(),
        &cache,
    );
    assert_eq!(plan.wire.len(), 2);
    assert!(plan.needed_projections.is_empty());
}

#[test]
fn unknown_mime_kind_is_marker_plus_text_projection() {
    let plan = plan(
        &[media_block("chemical/x-mdl-molfile", b"odd", &[])],
        vision_audio(),
        &no_cache(),
    );
    assert_eq!(
        plan.wire,
        vec![WireBlock::Marker(media_marker(
            "chemical/x-mdl-molfile",
            b"odd",
            &[],
        ))]
    );
    assert_eq!(
        plan.needed_projections,
        vec![(media_of(b"odd"), ProjectionKind::TextProjection)]
    );
}

#[test]
fn empty_meta_media_block_renders_deterministic_marker() {
    let block = austringer_core::event::content::MediaBlock::new(
        MimeType::new("model/gltf-binary").expect("valid"),
        media_of(IMAGE_BYTES),
        BTreeMap::new(),
    )
    .expect("bounded meta");
    let plan = plan(
        &[austringer_core::event::content::ContentBlock::Media(block)],
        audio(),
        &no_cache(),
    );
    assert_eq!(
        plan.wire,
        vec![WireBlock::Marker(media_marker(
            "model/gltf-binary",
            IMAGE_BYTES,
            &[],
        ))]
    );
}

#[test]
fn wire_block_text_rendering_is_deterministic_and_hash_keyed() {
    let native = WireBlock::NativeImage {
        mime: MimeType::png(),
        media: media_of(IMAGE_BYTES),
    };
    assert_eq!(
        native.text(),
        format!(
            "[image: image/png {}]",
            ContentHash::from_bytes(IMAGE_BYTES).short_ref()
        )
    );
    let audio_native = WireBlock::NativeAudio {
        mime: MimeType::new("audio/wav").expect("valid"),
        media: media_of(AUDIO_BYTES),
    };
    assert_eq!(
        audio_native.text(),
        format!(
            "[audio: audio/wav {}]",
            ContentHash::from_bytes(AUDIO_BYTES).short_ref()
        )
    );
    assert_eq!(WireBlock::Text("t".to_owned()).text(), "t");
    assert_eq!(WireBlock::Marker("m".to_owned()).text(), "m");
}
