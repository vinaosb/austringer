//! M10: model-mediated projection through the daemon's config-driven
//! provider seam (M32) — the projection inference composed over the real
//! family wire (cassette transport, zero network), provenance in the
//! derived store, zero recompute on the second submit, and the anti-fake
//! proof: the text-only session model's built prompt CONTAINS the
//! projection text (asserted on the captured request bytes).

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use austringer_core::agent_loop::{CancelToken, InferenceRequest};
use austringer_core::event::derived::DerivedStore;
use austringer_core::event::media::MediaStore;
use austringer_core::event::{
    ContentBlock, Event, ImageBlock, MediaBlock, MimeType, ProjectionKind, RecordedEvent, Sequence,
};
use austringer_core::media_delivery::DeliveryCapabilities;
use austringer_daemon::provider::{ProviderFamily, ProviderSelection};
use austringer_media::pipeline::{
    DELIVERY_CACHE_PRODUCER, MediatedProjection, Mediation, ProjectionOutcome, ProjectionResolver,
    Submission,
};
use austringer_media::projection::ProjectionRegistry;
use common::cassette::{Cassette, CassetteTransport, anthropic_text_cassette};

const KEY_ENV: &str = "AUSTRINGER_M10_PROJECTION_KEY";
const MODEL: &str = "claude-m10-cassette";
const PROJECTION_TEXT: &str = "three bands: red, green, and blue";
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nm10-fixture-payload";

/// Cache view that compares by value (DerivedCorrupt has no PartialEq).
fn derived_cache(
    derived: &DerivedStore,
    source: &austringer_core::event::ContentHash,
    producer: &str,
    kind: ProjectionKind,
) -> Option<Vec<u8>> {
    derived.load_derived(source, producer, kind).ok().flatten()
}

fn vision_caps() -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: true,
        audio_input: false,
    }
}

fn selection() -> ProviderSelection {
    ProviderSelection {
        family: ProviderFamily::Anthropic,
        model: MODEL.to_owned(),
        api_key_env: Some(KEY_ENV.to_owned()),
        base_url: None,
        inference_timeout: Duration::from_secs(30),
    }
}

fn image_blocks(session_dir: &std::path::Path) -> Vec<ContentBlock> {
    let store = MediaStore::open(session_dir).expect("media store");
    let reference = store.store_media(PNG_BYTES).expect("store fixture bytes");
    vec![ContentBlock::Image(
        ImageBlock::new("image/png", 16, 16, reference).expect("image block"),
    )]
}

/// The audio arm on the real wire: audio/wav is not in the anthropic
/// native allowlist, so even a vision model renders it as a marker —
/// this block exercises the Media (non-image) path end to end.
#[allow(dead_code)]
fn wav_block(session_dir: &std::path::Path) -> ContentBlock {
    let store = MediaStore::open(session_dir).expect("media store");
    let reference = store
        .store_media(b"RIFF----WAVEfmt m10")
        .expect("store wav bytes");
    ContentBlock::Media(
        MediaBlock::new(
            MimeType::new("audio/wav").expect("mime"),
            reference,
            BTreeMap::new(),
        )
        .expect("media block"),
    )
}

#[tokio::test]
async fn model_mediated_projection_composes_over_the_real_wire_with_provenance() {
    let session_dir = tempfile::tempdir().expect("session dir");
    let blocks = image_blocks(session_dir.path());
    let transport = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        MODEL,
        PROJECTION_TEXT,
    ))]);
    let resolved = selection()
        .resolve_with(|name| (name == KEY_ENV).then(|| "m10-fake-key".to_owned()))
        .expect("resolve selection");
    let factory = resolved
        .compose_projection_factory(transport.clone(), vision_caps(), session_dir.path())
        .expect("projection factory");

    let session = MediatedProjection {
        model_id: MODEL.to_owned(),
        caps: vision_caps(),
        factory,
        timeout: Duration::from_secs(10),
    };
    let registry = ProjectionRegistry::new(); // the shipped registry: no adapters yet
    let outcomes = ProjectionResolver::new(session_dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: DeliveryCapabilities::default(),
            },
            Mediation {
                session: Some(&session),
                utility: None,
            },
        )
        .await
        .expect("resolve completes");
    match &outcomes[0] {
        ProjectionOutcome::Resolved { producer, .. } => {
            assert_eq!(producer, &format!("model:{MODEL}:v1"));
        }
        other => panic!("cassette model must project: {other:?}"),
    }

    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "exactly one provider request");
    assert_eq!(
        requests[0].url, "https://api.anthropic.com/v1/messages",
        "the projection rides the real family endpoint"
    );
    let derived = DerivedStore::open(session_dir.path()).expect("derived store");
    let source = austringer_core::event::ContentHash::from_bytes(PNG_BYTES);
    assert_eq!(
        derived_cache(
            &derived,
            &source,
            &format!("model:{MODEL}:v1"),
            ProjectionKind::SceneDescription
        ),
        Some(PROJECTION_TEXT.as_bytes().to_vec()),
        "model-mediated result stored derived with model-id provenance"
    );
    assert_eq!(
        derived_cache(
            &derived,
            &source,
            DELIVERY_CACHE_PRODUCER,
            ProjectionKind::SceneDescription
        ),
        Some(PROJECTION_TEXT.as_bytes().to_vec())
    );
}

#[tokio::test]
async fn second_submit_resolves_from_cache_with_zero_model_calls() {
    let session_dir = tempfile::tempdir().expect("session dir");
    let blocks = image_blocks(session_dir.path());
    let resolved = selection()
        .resolve_with(|name| (name == KEY_ENV).then(|| "m10-fake-key".to_owned()))
        .expect("resolve selection");

    let seeding_transport = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        MODEL,
        PROJECTION_TEXT,
    ))]);
    let seeding = MediatedProjection {
        model_id: MODEL.to_owned(),
        caps: vision_caps(),
        factory: resolved
            .compose_projection_factory(seeding_transport, vision_caps(), session_dir.path())
            .expect("factory"),
        timeout: Duration::from_secs(10),
    };
    let registry = ProjectionRegistry::new();
    let first = ProjectionResolver::new(session_dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: DeliveryCapabilities::default(),
            },
            Mediation {
                session: Some(&seeding),
                utility: None,
            },
        )
        .await
        .expect("seed resolve");
    assert!(matches!(first[0], ProjectionOutcome::Resolved { .. }));

    // An EMPTY cassette deck: any model call would fail "cassette
    // exhausted" — resolving from the cache must make zero calls.
    let empty_deck = CassetteTransport::new(vec![]);
    let cached = MediatedProjection {
        model_id: MODEL.to_owned(),
        caps: vision_caps(),
        factory: resolved
            .compose_projection_factory(empty_deck.clone(), vision_caps(), session_dir.path())
            .expect("factory"),
        timeout: Duration::from_secs(10),
    };
    let second = ProjectionResolver::new(session_dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: DeliveryCapabilities::default(),
            },
            Mediation {
                session: Some(&cached),
                utility: None,
            },
        )
        .await
        .expect("second resolve");
    assert!(
        matches!(second[0], ProjectionOutcome::Resolved { .. }),
        "cache must answer: {second:?}"
    );
    assert!(
        empty_deck.requests().is_empty(),
        "COUNTER-PIN: second submit makes zero model calls"
    );
}

#[tokio::test]
async fn text_only_session_prompt_contains_the_projection_text() {
    let session_dir = tempfile::tempdir().expect("session dir");
    let blocks = image_blocks(session_dir.path());

    // Seed the derived store through the pipeline (cassette model).
    let resolved = selection()
        .resolve_with(|name| (name == KEY_ENV).then(|| "m10-fake-key".to_owned()))
        .expect("resolve selection");
    let seeding = MediatedProjection {
        model_id: MODEL.to_owned(),
        caps: vision_caps(),
        factory: resolved
            .compose_projection_factory(
                CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
                    MODEL,
                    PROJECTION_TEXT,
                ))]),
                vision_caps(),
                session_dir.path(),
            )
            .expect("factory"),
        timeout: Duration::from_secs(10),
    };
    let registry = ProjectionRegistry::new();
    ProjectionResolver::new(session_dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: DeliveryCapabilities::default(),
            },
            Mediation {
                session: Some(&seeding),
                utility: None,
            },
        )
        .await
        .expect("seed resolve");

    // The text-only session turn: same media block in the model context.
    // The adapter must render marker + inlined cached projection.
    let session_transport = CassetteTransport::new(vec![Cassette::Sse(anthropic_text_cassette(
        MODEL,
        "the tif has three bands",
    ))]);
    let text_only_session = resolved
        .compose_projection_factory(
            session_transport.clone(),
            DeliveryCapabilities::default(),
            session_dir.path(),
        )
        .expect("text-only factory");
    let mut inference = text_only_session();
    let events = [RecordedEvent {
        seq: Sequence::FIRST,
        event: Event::User {
            blocks: blocks.clone(),
        },
    }];
    let request = InferenceRequest {
        events: &events,
        focus_block: "how many bands does the image have?",
    };
    let turn = inference
        .infer(&request, &CancelToken::new())
        .await
        .expect("text-only session inference");

    let requests = session_transport.requests();
    assert_eq!(requests.len(), 1, "one session request");
    let prompt = String::from_utf8_lossy(&requests[0].body).to_string();
    assert!(
        prompt.contains(PROJECTION_TEXT),
        "ANTI-FAKE: the built prompt must contain the projection text: {prompt}"
    );
    assert!(
        prompt.contains("how many bands does the image have?"),
        "the focus block rides the prompt: {prompt}"
    );
    let answer: String = turn
        .blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image(_) | ContentBlock::Media(_) => None,
        })
        .collect();
    assert_eq!(answer, "the tif has three bands");
}

#[tokio::test]
async fn unprojection_under_no_mediation_keeps_the_marker_and_names_the_gap() {
    let session_dir = tempfile::tempdir().expect("session dir");
    let blocks = image_blocks(session_dir.path());
    let registry = ProjectionRegistry::new();
    let outcomes = ProjectionResolver::new(session_dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: DeliveryCapabilities::default(),
            },
            Mediation {
                session: None,
                utility: None,
            },
        )
        .await
        .expect("resolve completes");
    let outcome = &outcomes[0];
    match outcome {
        ProjectionOutcome::Unprojectable { mime, .. } => {
            assert_eq!(mime, "image/png");
            let notice = outcome.notice().expect("notice present");
            assert!(notice.contains("scene_description"), "{notice}");
        }
        other => panic!("no mediation must be unprojectable: {other:?}"),
    }
}
