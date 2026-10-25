//! M8 per-family cassette tests (hermetic, redacted fixtures): plan-driven
//! native media wire encoding for Anthropic (image source), OpenAI
//! (image_url + input_audio), Google (inline_data), and xAI (image_url
//! only, audio never reaches the wire).

mod common;

use std::sync::Arc;

use austringer_providers::adapter::InferenceAdapter;
use austringer_providers::adapters::{GoogleAdapter, OpenAiAdapter, XaiAdapter};
use austringer_providers::auth::StaticApiKey;
use austringer_providers::media::MediaDelivery;
use base64ct::{Base64, Encoding as _};
use common::inferkit::config;
use common::mediakit::{
    AUDIO_FIXTURE, IMAGE_FIXTURE, MediaSession, anthropic, anthropic_text_reply, chat_text_reply,
    gemini_text_reply, media_events, run, transport, vision_audio_caps,
};

#[tokio::test]
async fn anthropic_vision_model_receives_native_image_source() {
    let (session, image, _) = MediaSession::with_fixtures("m8-an").expect("session");
    let transport = transport(vec![anthropic_text_reply("claude-sonnet-4-6")]);
    let adapter = InferenceAdapter::new(anthropic(&transport, "claude-sonnet-4-6"), config(), 99)
        .with_capabilities(vision_audio_caps())
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(adapter, &media_events(image, None))
        .await
        .expect("completes");
    let body = &transport.bodies()[0];
    let blocks = body["messages"][0]["content"]
        .as_array()
        .expect("content becomes a block array");
    assert_eq!(blocks[0]["type"], "text");
    assert_eq!(blocks[1]["type"], "image");
    assert_eq!(blocks[1]["source"]["type"], "base64");
    assert_eq!(blocks[1]["source"]["media_type"], "image/png");
    assert_eq!(
        blocks[1]["source"]["data"],
        Base64::encode_string(IMAGE_FIXTURE)
    );
    assert!(
        blocks[0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("[image: image/png")),
        "native blocks still render a hash-keyed text marker in the prompt"
    );
}

#[tokio::test]
async fn openai_chat_receives_image_url_and_input_audio() {
    let (session, image, audio) = MediaSession::with_fixtures("m8-oa").expect("session");
    let transport = transport(vec![chat_text_reply("gpt-5-5")]);
    let provider = OpenAiAdapter::chat(
        transport.clone(),
        Arc::new(StaticApiKey::bearer("sk-oai")),
        "gpt-5-5",
    );
    let adapter = InferenceAdapter::new(Arc::new(provider), config(), 99)
        .with_capabilities(vision_audio_caps())
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(adapter, &media_events(image, Some(audio)))
        .await
        .expect("completes");
    let body = &transport.bodies()[0];
    let blocks = body["messages"][0]["content"]
        .as_array()
        .expect("content becomes a block array");
    assert_eq!(blocks[0]["type"], "text");
    assert_eq!(blocks[1]["type"], "image_url");
    assert_eq!(
        blocks[1]["image_url"]["url"],
        format!(
            "data:image/png;base64,{}",
            Base64::encode_string(IMAGE_FIXTURE)
        )
    );
    assert_eq!(blocks[2]["type"], "input_audio");
    assert_eq!(
        blocks[2]["input_audio"]["data"],
        Base64::encode_string(AUDIO_FIXTURE)
    );
    assert_eq!(blocks[2]["input_audio"]["format"], "wav");
}

#[tokio::test]
async fn google_receives_inline_data_for_image_and_audio() {
    let (session, image, audio) = MediaSession::with_fixtures("m8-go").expect("session");
    let transport = transport(vec![gemini_text_reply()]);
    let provider = GoogleAdapter::new(
        transport.clone(),
        Arc::new(StaticApiKey::bearer("g-key")),
        "gemini-3-pro",
    );
    let adapter = InferenceAdapter::new(Arc::new(provider), config(), 99)
        .with_capabilities(vision_audio_caps())
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(adapter, &media_events(image, Some(audio)))
        .await
        .expect("completes");
    let body = &transport.bodies()[0];
    let parts = body["contents"][0]["parts"]
        .as_array()
        .expect("parts array")
        .clone();
    assert!(
        parts[0]["text"]
            .as_str()
            .is_some_and(|t| t.starts_with("<focus>todos</focus>"))
    );
    assert_eq!(parts[1]["inline_data"]["mime_type"], "image/png");
    assert_eq!(
        parts[1]["inline_data"]["data"],
        Base64::encode_string(IMAGE_FIXTURE)
    );
    assert_eq!(parts[2]["inline_data"]["mime_type"], "audio/wav");
    assert_eq!(
        parts[2]["inline_data"]["data"],
        Base64::encode_string(AUDIO_FIXTURE)
    );
}

#[tokio::test]
async fn xai_receives_images_but_never_audio() {
    let (session, image, audio) = MediaSession::with_fixtures("m8-xa").expect("session");
    let transport = transport(vec![chat_text_reply("grok-4-6")]);
    let provider = XaiAdapter::new(
        transport.clone(),
        Arc::new(StaticApiKey::bearer("x-key")),
        "grok-4-6",
    );
    let adapter = InferenceAdapter::new(Arc::new(provider), config(), 99)
        .with_capabilities(vision_audio_caps())
        .with_media_delivery(MediaDelivery::open(session.path()).expect("delivery"));
    run(adapter, &media_events(image, Some(audio)))
        .await
        .expect("completes");
    let raw = &transport.raw_bodies()[0];
    assert!(raw.contains(&format!(
        "data:image/png;base64,{}",
        Base64::encode_string(IMAGE_FIXTURE)
    )));
    assert!(
        !raw.contains(&Base64::encode_string(AUDIO_FIXTURE)),
        "xAI has no native audio mimes; audio must never reach its wire"
    );
    assert!(!raw.contains("input_audio"));
    let body = &transport.bodies()[0];
    let text = body["messages"][0]["content"][0]["text"]
        .as_str()
        .expect("text block");
    assert!(
        text.contains("[audio: audio/wav"),
        "audio planned native under audio_input caps renders its hash-keyed marker"
    );
}
