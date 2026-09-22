//! M8 delivery guard tests: the incapable-model pin (byte-level absence
//! of base64), the fail-closed unknown family, the native-MIME data
//! table, typed missing-byte failure, and deterministic cached-projection
//! inlining (stale-state).

mod common;

use std::sync::Arc;

use base64ct::{Base64, Encoding as _};
use common::inferkit::config;
use common::mediakit::{
    IMAGE_FIXTURE, MediaSession, anthropic, anthropic_text_reply, chat_text_reply, image_hash,
    media_events, run, transport, vision_audio_caps,
};
use harness_core::event::content::{ContentBlock, ImageBlock};
use harness_core::event::derived::{DerivedStore, ProjectionKind};
use harness_core::event::media::MediaRef;
use harness_providers::adapter::InferenceAdapter;
use harness_providers::adapters::CompatAdapter;
use harness_providers::auth::StaticApiKey;
use harness_providers::media::{DELIVERY_PROJECTION_PRODUCER, MediaDelivery, native_mimes};

#[tokio::test]
async fn incapable_model_never_receives_base64_bytes() {
    let (session, image, audio) = MediaSession::with_fixtures("m8-inc").expect("session");
    let transport = transport(vec![anthropic_text_reply("claude-sonnet-4-6")]);
    // Default caps (text-only) with the store attached: the planner must
    // queue projections and render markers, NEVER native bytes.
    let adapter = InferenceAdapter::new(anthropic(&transport, "claude-sonnet-4-6"), config(), 99)
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(adapter, &media_events(image, Some(audio)))
        .await
        .expect("completes");
    let body = &transport.bodies()[0];
    assert!(
        body["messages"][0]["content"].is_string(),
        "text-only requests keep the plain-string content form"
    );
    let raw = &transport.raw_bodies()[0];
    assert!(
        !raw.contains(&Base64::encode_string(IMAGE_FIXTURE)),
        "no image base64 on the wire for an incapable model"
    );
    assert!(
        !raw.contains(&Base64::encode_string(common::mediakit::AUDIO_FIXTURE)),
        "no audio base64 on the wire for an incapable model"
    );
    assert!(!raw.contains("base64"));
    let raw_fixture = String::from_utf8_lossy(IMAGE_FIXTURE);
    assert!(
        !raw.contains(raw_fixture.as_ref()),
        "no raw media bytes either"
    );
    let prompt = body["messages"][0]["content"].as_str().expect("prompt");
    assert!(prompt.contains("[image: image/png 4x4 blake3:"));
    assert!(prompt.contains("[media: audio/wav blake3:"));
}

#[tokio::test]
async fn unknown_family_fails_closed_to_markers() {
    let (session, image, _) = MediaSession::with_fixtures("m8-cmp").expect("session");
    let transport = transport(vec![chat_text_reply("local-model")]);
    let provider = CompatAdapter::new(
        transport.clone(),
        Arc::new(StaticApiKey::bearer("k")),
        "http://127.0.0.1:11434/v1",
        "local-model",
    );
    let adapter = InferenceAdapter::new(Arc::new(provider), config(), 99)
        .with_capabilities(vision_audio_caps())
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(adapter, &media_events(image, None))
        .await
        .expect("completes");
    let raw = &transport.raw_bodies()[0];
    assert!(!raw.contains("base64"), "compat family has no native mimes");
    assert!(native_mimes("compat").is_empty());
    assert!(native_mimes("totally-unknown").is_empty());
}

#[test]
fn native_mime_table_matches_family_surfaces() {
    assert!(native_mimes("anthropic").contains(&"image/png"));
    assert!(
        !native_mimes("anthropic")
            .iter()
            .any(|mime| mime.starts_with("audio/"))
    );
    assert!(native_mimes("openai").contains(&"audio/mpeg"));
    assert!(native_mimes("google").contains(&"audio/flac"));
    assert!(native_mimes("xai").contains(&"image/jpeg"));
    assert!(
        !native_mimes("xai")
            .iter()
            .any(|mime| mime.starts_with("audio/"))
    );
}

#[tokio::test]
async fn missing_media_bytes_fail_typed_before_any_request() {
    let empty = MediaSession::empty("m8-missing").expect("session");
    let unstored = ImageBlock::new(
        "image/png",
        4,
        4,
        MediaRef {
            hash: image_hash(),
            bytes: IMAGE_FIXTURE.len() as u64,
        },
    )
    .expect("typed image construction");
    let transport = transport(Vec::new());
    let adapter = InferenceAdapter::new(anthropic(&transport, "claude-sonnet-4-6"), config(), 99)
        .with_capabilities(vision_audio_caps())
        .with_media_delivery(MediaDelivery::open(empty.path()).expect("delivery"));
    let failure = run(adapter, &media_events(ContentBlock::Image(unstored), None))
        .await
        .expect_err("missing bytes must fail typed");
    assert!(failure.to_string().contains("media blake3:"));
    assert!(transport.bodies().is_empty(), "nothing was sent");
}

#[tokio::test]
async fn cached_projection_inlines_and_updates_with_content() {
    let (session, image, _) = MediaSession::with_fixtures("m8-stale").expect("session");
    let derived = DerivedStore::open(session.path()).expect("derived");
    derived
        .store_derived(
            &image_hash(),
            DELIVERY_PROJECTION_PRODUCER,
            ProjectionKind::SceneDescription,
            b"scene: a red square",
        )
        .expect("store projection");
    let transport = transport(vec![
        anthropic_text_reply("claude-sonnet-4-6"),
        anthropic_text_reply("claude-sonnet-4-6"),
    ]);
    let events = media_events(image, None);
    let first = InferenceAdapter::new(anthropic(&transport, "claude-sonnet-4-6"), config(), 99)
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(first, &events).await.expect("completes");
    let bodies = transport.bodies();
    let prompt = bodies[0]["messages"][0]["content"]
        .as_str()
        .expect("prompt");
    assert!(prompt.contains("scene: a red square"));
    assert!(prompt.contains("[image: image/png 4x4 blake3:"));

    derived
        .store_derived(
            &image_hash(),
            DELIVERY_PROJECTION_PRODUCER,
            ProjectionKind::SceneDescription,
            b"scene: a blue circle",
        )
        .expect("updated projection");
    let second = InferenceAdapter::new(anthropic(&transport, "claude-sonnet-4-6"), config(), 99)
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(second, &events).await.expect("completes");
    let bodies = transport.bodies();
    let prompt = bodies[1]["messages"][0]["content"]
        .as_str()
        .expect("prompt");
    assert!(prompt.contains("scene: a blue circle"));
    assert!(!prompt.contains("scene: a red square"));
}
