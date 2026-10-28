//! Prefix-cache pins for [`InferenceAdapter`]'s history rendering: the
//! history region is byte-stable by the layout law, so a media-less
//! adapter may render each call as cached-prefix + new-suffix — but the
//! result must stay byte-identical to a full re-render in every shape:
//! appended events, a rewound (shorter) event list, a same-length
//! rewrite with re-sequenced events, and media delivery (where renders
//! consult live stores and a cache is structurally wrong).
//!
//! These pins fail on the bug classes an incremental cache can
//! introduce: stale prefix reuse after rewind, missing invalidation on
//! re-sequenced rewrites, separator drift between the cached and fresh
//! regions, and caching on the media path (a stale marker-only prompt
//! after a projection lands).

mod common;

use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use austringer_core::agent_loop::{CancelToken, Inference, InferenceRequest};
use austringer_core::event::derived::{DerivedStore, ProjectionKind};
use austringer_core::event::{Event, RecordedEvent};
use austringer_providers::adapter::{InferenceAdapter, assemble_wire_prompt, render_wire_history};
use austringer_providers::error::ProviderStreamError;
use austringer_providers::identity::{CompletionRequest, ModelIdentity};
use austringer_providers::media::{DELIVERY_PROJECTION_PRODUCER, MediaDelivery};
use austringer_providers::provider::{Provider, ProviderEventStream};
use austringer_providers::transport::TransportFailure;
use austringer_test_support::provider::FakeApi;
use austringer_test_support::sse::SseScript;
use common::inferkit::{self, config, identity, recorded, text_block};
use common::mediakit::{MediaSession, image_hash};
use common::{FakeBridge, Plan};

const FOCUS: &str = "<focus-chain>volatile tail</focus-chain>";

/// The full-render oracle: exactly the bytes a media-less adapter that
/// re-rendered the whole history from scratch would put on the wire.
fn oracle(events: &[RecordedEvent]) -> String {
    let rendered = render_wire_history(events).expect("full render oracle");
    assemble_wire_prompt(&rendered, FOCUS).0
}

async fn drive(infer: &mut InferenceAdapter, events: &[RecordedEvent]) {
    infer
        .infer(
            &InferenceRequest {
                events,
                focus_block: FOCUS,
                system: "",
                tools: &[],
            },
            &CancelToken::new(),
        )
        .await
        .expect("completes");
}

fn extended_tail() -> Vec<RecordedEvent> {
    vec![
        recorded(
            6,
            Event::Assistant {
                blocks: vec![text_block("window read")],
            },
        ),
        recorded(
            7,
            Event::ToolCall {
                call_id: "c6".to_owned(),
                tool: "view_window".to_owned(),
                arguments: serde_json::json!({ "path": "y" }),
            },
        ),
        recorded(
            8,
            Event::ToolResult {
                call_id: "c6".to_owned(),
                blocks: vec![text_block("lines 1-4")],
            },
        ),
        recorded(
            9,
            Event::System {
                text: "notice".to_owned(),
            },
        ),
    ]
}

#[tokio::test(start_paused = true)]
async fn prefix_cache_append_is_byte_identical_to_full_render() {
    let (bridge, mut infer) = inferkit::adapter(vec![
        Plan::ok(SseScript::single_text("ack")),
        Plan::ok(SseScript::single_text("ack")),
    ]);
    let base = inferkit::context();
    drive(&mut infer, &base).await;

    let mut extended = base.clone();
    extended.extend(extended_tail());
    drive(&mut infer, &extended).await;

    let served = bridge.served();
    assert_eq!(served.len(), 2);
    assert_eq!(served[0].prompt, oracle(&base));
    assert_eq!(
        served[1].prompt,
        oracle(&extended),
        "the cached incremental render must equal the full-render oracle byte-for-byte"
    );
}

/// Captures the full [`CompletionRequest`] (prompt AND native media),
/// then fails the send — the request shape is the only thing under test.
#[derive(Default)]
struct CaptureProvider {
    requests: Mutex<Vec<CompletionRequest>>,
}

#[async_trait]
impl Provider for CaptureProvider {
    fn identity(&self) -> &ModelIdentity {
        static IDENTITY: OnceLock<ModelIdentity> = OnceLock::new();
        IDENTITY.get_or_init(|| ModelIdentity::new("anthropic", "claude-pc"))
    }

    async fn stream_completion(
        &self,
        request: &CompletionRequest,
    ) -> Result<ProviderEventStream, ProviderStreamError> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request.clone());
        }
        Err(ProviderStreamError::Transport(TransportFailure::Protocol(
            "capture-only provider".into(),
        )))
    }
}

#[tokio::test(start_paused = true)]
async fn prefix_cache_append_native_media_matches_full_render() {
    // Retry-free: the capture provider fails every send, so exactly one
    // capture per infer call reaches the assertion below.
    let no_retry = austringer_providers::turn::RecoveryConfig {
        retry: austringer_providers::retry::RetryPolicy {
            max_attempts: 1,
            initial_backoff: std::time::Duration::from_millis(1),
            max_backoff: std::time::Duration::from_millis(1),
            multiplier: 1.0,
        },
        max_resumes: 0,
        idle_timeout: std::time::Duration::from_secs(1),
    };
    let provider = Arc::new(CaptureProvider::default());
    let mut infer = InferenceAdapter::new(Arc::clone(&provider) as Arc<dyn Provider>, no_retry, 99);
    let base = inferkit::context();
    let _ = infer
        .infer(
            &InferenceRequest {
                events: &base,
                focus_block: FOCUS,
                system: "",
                tools: &[],
            },
            &CancelToken::new(),
        )
        .await;
    let mut extended = base.clone();
    extended.extend(extended_tail());
    let _ = infer
        .infer(
            &InferenceRequest {
                events: &extended,
                focus_block: FOCUS,
                system: "",
                tools: &[],
            },
            &CancelToken::new(),
        )
        .await;

    let requests = provider.requests.lock().expect("capture").clone();
    assert_eq!(requests.len(), 2);
    for (request, events) in [&requests[0], &requests[1]].iter().zip([&base, &extended]) {
        assert_eq!(request.prompt, oracle(events));
        // A text-only adapter plans no native blocks, so the full
        // render's native vec is empty — the cached path must agree.
        assert!(request.media.is_empty());
    }
}

#[tokio::test(start_paused = true)]
async fn prefix_cache_rewind_to_shorter_history_invalidates() {
    let (bridge, mut infer) = inferkit::adapter(vec![
        Plan::ok(SseScript::single_text("ack")),
        Plan::ok(SseScript::single_text("ack")),
        Plan::ok(SseScript::single_text("ack")),
    ]);
    let base = inferkit::context(); // 5 events, seq 1..=5 -> caches through 5
    drive(&mut infer, &base).await;

    // Rewind: the list shrinks below the cached watermark (compaction /
    // snapshot restore), then a fresh event re-uses sequence position 4.
    let mut rewound: Vec<_> = base[..3].to_vec();
    rewound.push(recorded(
        4,
        Event::User {
            blocks: vec![text_block("fresh start after rewind")],
        },
    ));
    drive(&mut infer, &rewound).await;

    // Regrow past the OLD watermark: drop -> rebuild -> extend.
    let mut regrown = rewound.clone();
    regrown.push(recorded(
        5,
        Event::Assistant {
            blocks: vec![text_block("ack")],
        },
    ));
    regrown.push(recorded(
        6,
        Event::System {
            text: "again".to_owned(),
        },
    ));
    drive(&mut infer, &regrown).await;

    let served = bridge.served();
    assert_eq!(served.len(), 3);
    assert_eq!(served[0].prompt, oracle(&base));
    assert_eq!(
        served[1].prompt,
        oracle(&rewound),
        "a shorter event list must drop the cache and fully re-render"
    );
    assert_eq!(served[2].prompt, oracle(&regrown));
    assert_ne!(served[1].prompt, served[0].prompt);
}

#[tokio::test(start_paused = true)]
async fn prefix_cache_same_length_rewrite_shifted_seqs_invalidates() {
    let (bridge, mut infer) = inferkit::adapter(vec![
        Plan::ok(SseScript::single_text("ack")),
        Plan::ok(SseScript::single_text("ack")),
    ]);
    let base = inferkit::context(); // 5 events, seq 1..=5 -> caches through 5
    drive(&mut infer, &base).await;

    // Compaction-retained shape: SAME length (the length guard alone
    // would pass), prefix kept, tail replaced by re-sequenced events
    // (the gap at 6 is the tell — the boundary event is no longer the
    // event the cache rendered).
    let mut rewritten: Vec<_> = base[..3].to_vec();
    rewritten.push(recorded(
        7,
        Event::User {
            blocks: vec![text_block("compacted replacement u")],
        },
    ));
    rewritten.push(recorded(
        8,
        Event::Assistant {
            blocks: vec![text_block("compacted replacement a")],
        },
    ));
    drive(&mut infer, &rewritten).await;

    let served = bridge.served();
    assert_eq!(served.len(), 2);
    assert_ne!(served[0].prompt, served[1].prompt);
    assert_eq!(
        served[1].prompt,
        oracle(&rewritten),
        "a boundary sequence mismatch must drop the cache, not replay the stale tail"
    );
}

#[tokio::test(start_paused = true)]
async fn prefix_cache_media_delivery_rerenders_on_projections() {
    let (session, image, _) = MediaSession::with_fixtures("prefix-cache-media").expect("session");
    let delivery = MediaDelivery::open(session.path()).expect("delivery");
    let derived = DerivedStore::open(session.path()).expect("derived");
    let bridge = FakeBridge::new(
        Arc::new(FakeApi::new()),
        identity(),
        vec![
            Plan::ok(SseScript::single_text("ack")),
            Plan::ok(SseScript::single_text("ack")),
        ],
    );
    let mut infer =
        InferenceAdapter::new(Arc::new(bridge.clone()), config(), 7).with_media_delivery(delivery);

    let events = common::mediakit::media_events(image, None);
    drive(&mut infer, &events).await; // marker-only: no projection stored yet
    derived
        .store_derived(
            &image_hash(),
            DELIVERY_PROJECTION_PRODUCER,
            ProjectionKind::SceneDescription,
            b"scene: a red square with a white border",
        )
        .expect("projection lands");
    drive(&mut infer, &events).await; // SAME events, different store state

    let served = bridge.served();
    assert_eq!(served.len(), 2);
    assert!(
        served[0].prompt.contains("[image: image/png 4x4 blake3:"),
        "no projection yet: the deterministic marker renders"
    );
    assert!(
        !served[0].prompt.contains("scene: a red square"),
        "pre-arrival prompt must not carry the projection"
    );
    assert!(
        served[1]
            .prompt
            .contains("scene: a red square with a white border"),
        "media-attached adapters re-render from the live stores every call; \
         a prefix cache would replay the stale marker-only prompt"
    );
}
