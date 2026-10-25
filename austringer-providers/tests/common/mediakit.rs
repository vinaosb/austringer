//! Shared kit for M8 media-delivery tests: the request-recording
//! transport (serves scripted SSE replies), the redacted media fixtures,
//! and minimal per-family happy-path scripts.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use austringer_core::agent_loop::{CancelToken, Inference, InferenceRequest};
use austringer_core::event::content::{ContentBlock, MediaBlock, MimeType};
use austringer_core::event::media::{ContentHash, MediaRef, MediaStore};
use austringer_core::media_delivery::DeliveryCapabilities;
use austringer_providers::adapter::InferenceAdapter;
use austringer_providers::adapters::AnthropicAdapter;
use austringer_providers::auth::StaticApiKey;
use austringer_test_support::sse::{SseByteStream, SseScript, SseScriptBuilder};
use austringer_test_support::util::TestDir;

use super::frames::{cc_done, cc_finish, cc_text, cc_usage, gm_finish, gm_text};

/// Redacted image fixture bytes (synthetic placeholder, never real media).
pub const IMAGE_FIXTURE: &[u8] = include_bytes!("../fixtures/media/redacted-image.bin");
/// Redacted audio fixture bytes (synthetic placeholder, never real media).
pub const AUDIO_FIXTURE: &[u8] = include_bytes!("../fixtures/media/redacted-audio.bin");

/// A session directory with the media + derived stores the delivery
/// planner reads at request-build time.
pub struct MediaSession {
    #[allow(dead_code)]
    dir: TestDir,
}

impl MediaSession {
    /// An empty session (no stored bytes): references miss the store.
    pub fn empty(tag: &str) -> std::io::Result<Self> {
        Ok(Self {
            dir: TestDir::new(tag)?,
        })
    }

    /// Creates the session and stores both fixtures, returning the
    /// session path plus the stored blocks.
    pub fn with_fixtures(tag: &str) -> std::io::Result<(Self, ContentBlock, ContentBlock)> {
        let dir = TestDir::new(tag)?;
        let store = MediaStore::open(dir.path())?;
        let image = ContentBlock::Image(
            austringer_core::event::content::ImageBlock::new(
                "image/png",
                4,
                4,
                store_media(&store, IMAGE_FIXTURE)?,
            )
            .expect("typed image construction"),
        );
        let mut no_meta = std::collections::BTreeMap::new();
        no_meta.insert("codec".to_owned(), "pcm_s16".to_owned());
        let audio_ref = store_media(&store, AUDIO_FIXTURE)?;
        let audio = ContentBlock::Media(
            MediaBlock::new(
                MimeType::new("audio/wav").expect("valid mime"),
                audio_ref,
                no_meta,
            )
            .expect("bounded meta"),
        );
        Ok((Self { dir }, image, audio))
    }

    /// The session directory (open `MediaStore`/`DerivedStore` on it).
    pub fn path(&self) -> &std::path::Path {
        self.dir.path()
    }
}

fn store_media(store: &MediaStore, bytes: &[u8]) -> std::io::Result<MediaRef> {
    store.store_media(bytes)
}

/// Blake3 of the image fixture (input-derived key for assertions).
pub fn image_hash() -> ContentHash {
    ContentHash::from_bytes(IMAGE_FIXTURE)
}

/// Blake3 of the audio fixture (input-derived key for assertions).
pub fn audio_hash() -> ContentHash {
    ContentHash::from_bytes(AUDIO_FIXTURE)
}

/// Minimal Anthropic happy reply: one text delta, end_turn.
pub fn anthropic_text_reply(model: &str) -> SseScript {
    use austringer_test_support::sse::ev_message_stop;
    SseScriptBuilder::new()
        .event(super::frames::an_message_start(model, 10, 0, 0))
        .event(super::frames::an_text_delta("ok"))
        .event(super::frames::an_message_delta(1, "end_turn"))
        .event(ev_message_stop())
        .build()
}

/// Minimal chat-completions happy reply: text, stop, usage, [DONE].
pub fn chat_text_reply(model: &str) -> SseScript {
    SseScriptBuilder::new()
        .event(cc_text(model, "ok"))
        .event(cc_finish(model, "stop"))
        .event(cc_usage(model, 10, 1, 0))
        .event(cc_done())
        .build()
}

/// Minimal Gemini happy reply: one text part, STOP.
pub fn gemini_text_reply() -> SseScript {
    SseScriptBuilder::new()
        .event(gm_text("ok"))
        .event(gm_finish("STOP"))
        .build()
}

struct ScriptSource(SseByteStream);

#[async_trait]
impl austringer_providers::transport::ByteSource for ScriptSource {
    async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        self.0.next_chunk().await
    }
}

/// Records every rendered `TransportRequest` body and serves the queued
/// scripted replies in order (the cassette).
pub struct RecordingTransport {
    requests: Mutex<Vec<Vec<u8>>>,
    replies: Mutex<VecDeque<SseScript>>,
}

impl RecordingTransport {
    pub fn new(replies: Vec<SseScript>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            replies: Mutex::new(replies.into()),
        }
    }

    /// JSON-decoded bodies of every request sent, in order.
    pub fn bodies(&self) -> Vec<serde_json::Value> {
        self.requests
            .lock()
            .ok()
            .map(|bodies| {
                bodies
                    .iter()
                    .filter_map(|raw| serde_json::from_slice(raw).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Raw bodies as UTF-8 lossy strings (for byte-level absence checks).
    pub fn raw_bodies(&self) -> Vec<String> {
        self.requests
            .lock()
            .ok()
            .map(|bodies| {
                bodies
                    .iter()
                    .map(|raw| String::from_utf8_lossy(raw).into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[async_trait]
impl austringer_providers::transport::Transport for RecordingTransport {
    async fn send(
        &self,
        request: austringer_providers::transport::TransportRequest,
    ) -> Result<
        austringer_providers::transport::TransportResponse,
        austringer_providers::transport::TransportFailure,
    > {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request.body.clone());
        }
        let script = self
            .replies
            .lock()
            .ok()
            .and_then(|mut queue| queue.pop_front())
            .ok_or_else(|| {
                austringer_providers::transport::TransportFailure::Protocol(
                    "recording transport exhausted".to_owned(),
                )
            })?;
        Ok(austringer_providers::transport::TransportResponse::ok(
            austringer_providers::transport::Body::Stream(
                austringer_providers::transport::ByteStream::new(Box::new(ScriptSource(
                    SseByteStream::from_script(&script),
                ))),
            ),
        ))
    }
}

/// Arc alias for adapter construction.
pub fn transport(replies: Vec<SseScript>) -> Arc<RecordingTransport> {
    Arc::new(RecordingTransport::new(replies))
}

/// Capability fixture: every native input allowed.
pub fn vision_audio_caps() -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: true,
        audio_input: true,
    }
}

/// A one-event user context: text + optional media blocks.
pub fn media_events(
    image: ContentBlock,
    audio: Option<ContentBlock>,
) -> Vec<austringer_core::event::RecordedEvent> {
    let mut blocks = vec![super::inferkit::text_block("describe this media"), image];
    if let Some(audio) = audio {
        blocks.push(audio);
    }
    vec![super::inferkit::recorded(
        1,
        austringer_core::event::Event::User { blocks },
    )]
}

/// Drives one inference over `events` and discards the turn.
pub async fn run(
    mut adapter: InferenceAdapter,
    events: &[austringer_core::event::RecordedEvent],
) -> Result<(), austringer_core::agent_loop::InferenceError> {
    adapter
        .infer(
            &InferenceRequest {
                events,
                focus_block: "<focus>todos</focus>",
            },
            &CancelToken::new(),
        )
        .await?;
    Ok(())
}

/// An Anthropic adapter over the recording transport.
pub fn anthropic(transport: &Arc<RecordingTransport>, model: &str) -> Arc<AnthropicAdapter> {
    let carrier = Arc::clone(transport);
    Arc::new(AnthropicAdapter::new(
        carrier,
        Arc::new(StaticApiKey::x_api_key("sk-ant")),
        model,
    ))
}
