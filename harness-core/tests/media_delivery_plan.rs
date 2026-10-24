//! Delivery-planner table tests (plan M8): the exhaustive capability x
//! block-kind matrix plus ordering, dedup, and the empty-input edge. All
//! expectations are derived from the fixtures' inputs, never from the
//! planner's own output.

mod common;

use common::deliverykit::{
    AUDIO_BYTES, GLTF_BYTES, IMAGE_BYTES, PDF_BYTES, audio, image_block, image_marker, media_block,
    media_marker, media_of, no_cache, text_block, text_only, vision, vision_audio,
};
use harness_core::event::content::MimeType;
use harness_core::event::derived::ProjectionKind;
use harness_core::media_delivery::{DeliveryPlan, WireBlock, plan};

#[test]
fn exhaustive_capability_x_block_kind_table() {
    struct Row {
        name: &'static str,
        block: harness_core::event::content::ContentBlock,
        caps: harness_core::media_delivery::DeliveryCapabilities,
        wire: Vec<WireBlock>,
        needed: Vec<(harness_core::event::media::MediaRef, ProjectionKind)>,
    }
    let rows = vec![
        Row {
            name: "text any caps",
            block: text_block("hello"),
            caps: text_only(),
            wire: vec![WireBlock::Text("hello".to_owned())],
            needed: Vec::new(),
        },
        Row {
            name: "text vision caps",
            block: text_block("hello"),
            caps: vision_audio(),
            wire: vec![WireBlock::Text("hello".to_owned())],
            needed: Vec::new(),
        },
        Row {
            name: "image + vision -> native",
            block: image_block(),
            caps: vision(),
            wire: vec![WireBlock::NativeImage {
                mime: MimeType::png(),
                media: media_of(IMAGE_BYTES),
            }],
            needed: Vec::new(),
        },
        Row {
            name: "image + vision+audio -> native",
            block: image_block(),
            caps: vision_audio(),
            wire: vec![WireBlock::NativeImage {
                mime: MimeType::png(),
                media: media_of(IMAGE_BYTES),
            }],
            needed: Vec::new(),
        },
        Row {
            name: "image w/o vision -> marker + scene description",
            block: image_block(),
            caps: text_only(),
            wire: vec![WireBlock::Marker(image_marker())],
            needed: vec![(media_of(IMAGE_BYTES), ProjectionKind::SceneDescription)],
        },
        Row {
            name: "image + audio-only caps -> marker + scene description",
            block: image_block(),
            caps: audio(),
            wire: vec![WireBlock::Marker(image_marker())],
            needed: vec![(media_of(IMAGE_BYTES), ProjectionKind::SceneDescription)],
        },
        Row {
            name: "media image + vision -> native",
            block: media_block("image/png", IMAGE_BYTES, &[]),
            caps: vision(),
            wire: vec![WireBlock::NativeImage {
                mime: MimeType::png(),
                media: media_of(IMAGE_BYTES),
            }],
            needed: Vec::new(),
        },
        Row {
            name: "media image w/o vision -> marker + scene description",
            block: media_block("image/webp", IMAGE_BYTES, &[]),
            caps: text_only(),
            wire: vec![WireBlock::Marker(media_marker(
                "image/webp",
                IMAGE_BYTES,
                &[],
            ))],
            needed: vec![(media_of(IMAGE_BYTES), ProjectionKind::SceneDescription)],
        },
        Row {
            name: "audio + audio_input -> native",
            block: media_block("audio/wav", AUDIO_BYTES, &[]),
            caps: audio(),
            wire: vec![WireBlock::NativeAudio {
                mime: MimeType::new("audio/wav").expect("valid"),
                media: media_of(AUDIO_BYTES),
            }],
            needed: Vec::new(),
        },
        Row {
            name: "audio + vision+audio -> native",
            block: media_block("audio/wav", AUDIO_BYTES, &[]),
            caps: vision_audio(),
            wire: vec![WireBlock::NativeAudio {
                mime: MimeType::new("audio/wav").expect("valid"),
                media: media_of(AUDIO_BYTES),
            }],
            needed: Vec::new(),
        },
        Row {
            name: "audio w/o audio_input -> marker + transcript",
            block: media_block("audio/ogg", AUDIO_BYTES, &[]),
            caps: text_only(),
            wire: vec![WireBlock::Marker(media_marker(
                "audio/ogg",
                AUDIO_BYTES,
                &[],
            ))],
            needed: vec![(media_of(AUDIO_BYTES), ProjectionKind::Transcript)],
        },
        Row {
            name: "audio + vision-only caps -> marker + transcript",
            block: media_block("audio/wav", AUDIO_BYTES, &[]),
            caps: vision(),
            wire: vec![WireBlock::Marker(media_marker(
                "audio/wav",
                AUDIO_BYTES,
                &[],
            ))],
            needed: vec![(media_of(AUDIO_BYTES), ProjectionKind::Transcript)],
        },
        Row {
            name: "gltf + vision -> marker + text projection (honest)",
            block: media_block("model/gltf+json", GLTF_BYTES, &[]),
            caps: vision_audio(),
            wire: vec![WireBlock::Marker(media_marker(
                "model/gltf+json",
                GLTF_BYTES,
                &[],
            ))],
            needed: vec![(media_of(GLTF_BYTES), ProjectionKind::TextProjection)],
        },
        Row {
            name: "pdf text-only -> marker + text projection",
            block: media_block("application/pdf", PDF_BYTES, &[("pages", "3")]),
            caps: text_only(),
            wire: vec![WireBlock::Marker(media_marker(
                "application/pdf",
                PDF_BYTES,
                &[("pages", "3")],
            ))],
            needed: vec![(media_of(PDF_BYTES), ProjectionKind::TextProjection)],
        },
        Row {
            name: "video + vision -> marker + text projection (no native video in v0.2)",
            block: media_block("video/webm", AUDIO_BYTES, &[]),
            caps: vision_audio(),
            wire: vec![WireBlock::Marker(media_marker(
                "video/webm",
                AUDIO_BYTES,
                &[],
            ))],
            needed: vec![(media_of(AUDIO_BYTES), ProjectionKind::TextProjection)],
        },
    ];
    for row in rows {
        let DeliveryPlan {
            wire,
            needed_projections,
        } = plan(std::slice::from_ref(&row.block), row.caps, &no_cache());
        assert_eq!(wire, row.wire, "wire mismatch in row {}", row.name);
        assert_eq!(
            needed_projections, row.needed,
            "needed mismatch in row {}",
            row.name
        );
    }
}

#[test]
fn needed_projections_dedup_identical_media() {
    let blocks = vec![
        image_block(),
        media_block("image/png", IMAGE_BYTES, &[]),
        image_block(),
    ];
    let plan = plan(&blocks, text_only(), &no_cache());
    assert_eq!(
        plan.needed_projections,
        vec![(media_of(IMAGE_BYTES), ProjectionKind::SceneDescription)],
        "same content hash queued once per kind"
    );
    assert_eq!(plan.wire.len(), 3, "every block still renders");
}

#[test]
fn mixed_blocks_render_in_order() {
    let blocks = vec![
        text_block("look at this"),
        image_block(),
        text_block("and listen"),
        media_block("audio/wav", AUDIO_BYTES, &[]),
    ];
    let plan = plan(&blocks, vision(), &no_cache());
    assert_eq!(
        plan.wire,
        vec![
            WireBlock::Text("look at this".to_owned()),
            WireBlock::NativeImage {
                mime: MimeType::png(),
                media: media_of(IMAGE_BYTES),
            },
            WireBlock::Text("and listen".to_owned()),
            WireBlock::Marker(media_marker("audio/wav", AUDIO_BYTES, &[])),
        ]
    );
    assert_eq!(
        plan.needed_projections,
        vec![(media_of(AUDIO_BYTES), ProjectionKind::Transcript)]
    );
}

#[test]
fn empty_block_list_plans_to_empty() {
    let plan = plan(&[], vision_audio(), &no_cache());
    assert!(plan.wire.is_empty());
    assert!(plan.needed_projections.is_empty());
}
