//! M10: the submit-time projection resolve pipeline — derived-cache hit,
//! deterministic projector, capability-gated model mediation with
//! provenance, typed NoProjector notices, and the adversarial arms
//! (corrupt input, hung model, cache determinism).

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use harness_core::agent_loop::{
    AssistantTurn, CancelToken, Inference, InferenceError, InferenceRequest, PendingCall,
};
use harness_core::event::derived::DerivedStore;
use harness_core::event::media::MediaStore;
use harness_core::event::{ContentBlock, ImageBlock, ProjectionKind};
use harness_core::media_delivery::DeliveryCapabilities;
use harness_media::error::MediaError;
use harness_media::format::FormatId;
use harness_media::pipeline::{
    DELIVERY_CACHE_PRODUCER, MediatedProjection, Mediation, ProjectionOutcome, ProjectionResolver,
    Submission, Unprojectable,
};
use harness_media::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};
use tempfile::TempDir;

// ---- scripted model (hermetic: no network, no keys) ----

#[derive(Clone)]
enum Answer {
    Text(&'static str),
    Empty,
    ToolCall,
    Hang,
}

#[derive(Clone)]
struct ScriptedModel {
    answer: Answer,
    calls: Arc<std::sync::atomic::AtomicUsize>,
    focus_seen: Arc<std::sync::Mutex<Vec<String>>>,
}

impl ScriptedModel {
    fn new(answer: Answer) -> Self {
        Self {
            answer,
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            focus_seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn mediation(self, model_id: &str, caps: DeliveryCapabilities) -> MediatedProjection {
        let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
            Arc::new(move || Box::new(self.clone()));
        MediatedProjection {
            model_id: model_id.to_owned(),
            caps,
            factory,
            timeout: Duration::from_secs(5),
        }
    }
}

#[async_trait]
impl Inference for ScriptedModel {
    async fn infer(
        &mut self,
        request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut seen) = self.focus_seen.lock() {
            seen.push(request.focus_block.to_owned());
        }
        match self.answer.clone() {
            Answer::Text(text) => Ok(AssistantTurn {
                blocks: vec![ContentBlock::Text { text: text.to_owned() }],
                tool_calls: Vec::new(),
                usage: None,
            }),
            Answer::Empty => Ok(AssistantTurn {
                blocks: Vec::new(),
                tool_calls: Vec::new(),
                usage: None,
            }),
            Answer::ToolCall => Ok(AssistantTurn {
                blocks: Vec::new(),
                tool_calls: vec![PendingCall {
                    call_id: "call_1".to_owned(),
                    tool: "look".to_owned(),
                    arguments: serde_json::json!({}),
                }],
                usage: None,
            }),
            Answer::Hang => {
                std::future::pending::<Result<AssistantTurn, InferenceError>>().await
            }
        }
    }
}

// ---- deterministic projector ----

struct CountingProjector {
    kind: ProjectionKind,
    calls: std::sync::atomic::AtomicUsize,
}

impl ProjectionProvider for CountingProjector {
    fn kind(&self) -> ProjectionKind {
        self.kind
    }

    fn project(&self, _bytes: &[u8], _ctx: &ProjectionContext<'_>) -> Result<String, MediaError> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok("deterministic: 3 bands, 8x8".to_owned())
    }
}

// ---- fixtures ----

struct Fixture {
    dir: TempDir,
    registry: ProjectionRegistry,
}

fn fixture() -> Fixture {
    Fixture {
        dir: tempfile::tempdir().expect("temp session dir"),
        registry: ProjectionRegistry::new(),
    }
}

fn image_blocks(dir: &Path) -> Vec<ContentBlock> {
    let store = MediaStore::open(dir).expect("media store");
    let reference = store
        .store_media(b"\x89PNG\r\n\x1a\nfixture-payload")
        .expect("store png bytes");
    let block = ImageBlock::new("image/png", 8, 8, reference).expect("image block");
    vec![ContentBlock::Image(block)]
}

fn no_mediation<'a>() -> Mediation<'a> {
    Mediation {
        session: None,
        utility: None,
    }
}

fn text_only() -> DeliveryCapabilities {
    DeliveryCapabilities::default()
}

fn vision() -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: true,
        audio_input: false,
    }
}

fn resolved(outcome: &ProjectionOutcome) -> bool {
    matches!(outcome, ProjectionOutcome::Resolved { .. })
}

/// Cache view that compares by value (DerivedCorrupt has no PartialEq;
/// corrupt entries surface as `None` here and as typed reasons inside
/// the resolver).
fn derived_cache(
    derived: &DerivedStore,
    source: &harness_core::event::ContentHash,
    producer: &str,
    kind: ProjectionKind,
) -> Option<Vec<u8>> {
    derived.load_derived(source, producer, kind).ok().flatten()
}

#[tokio::test]
async fn deterministic_projection_caches_second_submit_zero_recompute() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    let projector = Arc::new(CountingProjector {
        kind: ProjectionKind::SceneDescription,
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let mut registry = ProjectionRegistry::new();
    registry
        .register(
            FormatId::Png,
            Arc::clone(&projector) as Arc<dyn ProjectionProvider>,
        )
        .expect("register projector");

    let first = ProjectionResolver::new(fix.dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            no_mediation(),
        )
        .await
        .expect("first resolve");
    assert_eq!(first.len(), 1, "one queued projection: {first:?}");
    match &first[0] {
        ProjectionOutcome::Resolved { producer, .. } => {
            assert_eq!(producer, "deterministic:png");
        }
        other => panic!("first submit must resolve deterministically: {other:?}"),
    }
    assert_eq!(
        projector.calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "first submit computes once"
    );

    let derived = DerivedStore::open(fix.dir.path()).expect("derived store");
    let source = harness_core::event::ContentHash::from_bytes(b"\x89PNG\r\n\x1a\nfixture-payload");
    assert_eq!(
        derived_cache(&derived, &source, "deterministic:png", ProjectionKind::SceneDescription),
        Some(b"deterministic: 3 bands, 8x8".to_vec())
    );
    assert_eq!(
        derived_cache(&derived, &source, DELIVERY_CACHE_PRODUCER, ProjectionKind::SceneDescription),
        Some(b"deterministic: 3 bands, 8x8".to_vec()),
        "the delivery alias must exist for request-build inlining"
    );

    let second = ProjectionResolver::new(fix.dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            no_mediation(),
        )
        .await
        .expect("second resolve");
    assert!(resolved(&second[0]), "second submit resolves: {second:?}");
    assert_eq!(
        projector.calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "COUNTER-PIN: second submit must recompute nothing"
    );
}

#[tokio::test]
async fn no_projector_anywhere_is_typed_notice_and_nothing_stored() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            no_mediation(),
        )
        .await
        .expect("resolve completes without crashing the submit");
    let outcome = &outcomes[0];
    match outcome {
        ProjectionOutcome::Unprojectable { mime, reason, .. } => {
            assert!(matches!(reason, Unprojectable::NoProjector), "{reason:?}");
            assert_eq!(mime, "image/png");
            let notice = outcome
                .notice()
                .expect("notice names what is missing");
            assert!(notice.contains("image/png"), "notice names the mime: {notice}");
            assert!(
                notice.contains("scene_description"),
                "notice names the kind: {notice}"
            );
        }
        other => panic!("no projector anywhere must be unprojectable: {other:?}"),
    }
    let derived = DerivedStore::open(fix.dir.path()).expect("derived store");
    let source = harness_core::event::ContentHash::from_bytes(b"\x89PNG\r\n\x1a\nfixture-payload");
    assert_eq!(
        derived_cache(&derived, &source, DELIVERY_CACHE_PRODUCER, ProjectionKind::SceneDescription),
        None,
        "nothing may be stored without a real projection"
    );
}

#[tokio::test]
async fn audio_with_no_projector_anywhere_keeps_marker_and_never_invents_a_transcript() {
    let fix = fixture();
    let store = MediaStore::open(fix.dir.path()).expect("media store");
    let reference = store
        .store_media(b"RIFF----WAVEfmt fixture")
        .expect("store wav bytes");
    let blocks = vec![ContentBlock::Media(
        harness_core::event::MediaBlock::new(
            harness_core::event::MimeType::new("audio/wav").expect("mime"),
            reference,
            std::collections::BTreeMap::new(),
        )
        .expect("media block"),
    )];
    let outcome = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            no_mediation(),
        )
        .await
        .expect("resolve completes")[0]
        .clone();
    match &outcome {
        ProjectionOutcome::Unprojectable {
            mime, kind, reason, ..
        } => {
            assert_eq!(mime, "audio/wav");
            assert_eq!(*kind, ProjectionKind::Transcript);
            assert!(matches!(reason, Unprojectable::NoProjector));
        }
        other => panic!("audio with no projector must be unprojectable: {other:?}"),
    }
    let notice = outcome.notice().expect("notice names the gap");
    assert!(notice.contains("audio/wav"), "{notice}");
    assert!(notice.contains("transcript"), "{notice}");
    let derived = DerivedStore::open(fix.dir.path()).expect("derived store");
    let source = harness_core::event::ContentHash::from_bytes(b"RIFF----WAVEfmt fixture");
    assert_eq!(
        derived_cache(&derived, &source, DELIVERY_CACHE_PRODUCER, ProjectionKind::Transcript),
        None,
        "the transcript is never invented"
    );
}

#[tokio::test]
async fn corrupt_media_is_typed_parse_failure_and_retries_for_repair() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    struct Failing;
    impl ProjectionProvider for Failing {
        fn kind(&self) -> ProjectionKind {
            ProjectionKind::SceneDescription
        }
        fn project(&self, _b: &[u8], _c: &ProjectionContext<'_>) -> Result<String, MediaError> {
            Err(MediaError::ParseFailure)
        }
    }
    let mut registry = ProjectionRegistry::new();
    registry
        .register(FormatId::Png, Arc::new(Failing))
        .expect("register failing projector");

    let outcomes = ProjectionResolver::new(fix.dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            no_mediation(),
        )
        .await
        .expect("resolve completes");
    match &outcomes[0] {
        ProjectionOutcome::Unprojectable { reason, .. } => {
            assert!(
                matches!(reason, Unprojectable::Deterministic(MediaError::ParseFailure)),
                "corrupt media must be typed, never fabricated: {reason:?}"
            );
        }
        other => panic!("corrupt media must be unprojectable: {other:?}"),
    }
    let again = ProjectionResolver::new(fix.dir.path(), &registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            no_mediation(),
        )
        .await
        .expect("second resolve");
    assert!(matches!(
        &again[0],
        ProjectionOutcome::Unprojectable {
            reason: Unprojectable::Deterministic(MediaError::ParseFailure),
            ..
        }
    ));
}

#[tokio::test]
async fn model_mediated_projection_lands_derived_with_provenance() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    let model = ScriptedModel::new(Answer::Text("a red square with three bands"));
    let session = model.clone().mediation("fixture-vision-model", vision());

    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
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
            assert_eq!(producer, "model:fixture-vision-model:v1");
        }
        other => panic!("capable session model must project: {other:?}"),
    }
    assert_eq!(model.calls(), 1, "exactly one model call");

    let derived = DerivedStore::open(fix.dir.path()).expect("derived store");
    let source = harness_core::event::ContentHash::from_bytes(b"\x89PNG\r\n\x1a\nfixture-payload");
    assert_eq!(
        derived_cache(
            &derived,
            &source,
            "model:fixture-vision-model:v1",
            ProjectionKind::SceneDescription
        ),
        Some(b"a red square with three bands".to_vec()),
        "provenance producer must carry model id + prompt params version"
    );
    assert_eq!(
        derived_cache(&derived, &source, DELIVERY_CACHE_PRODUCER, ProjectionKind::SceneDescription),
        Some(b"a red square with three bands".to_vec())
    );
}

#[tokio::test]
async fn incapable_session_falls_back_to_capable_utility_without_a_blind_call() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    let session_model = ScriptedModel::new(Answer::Text("session must never be called"));
    let session = session_model
        .clone()
        .mediation("text-only-session-model", text_only());
    let utility_model = ScriptedModel::new(Answer::Text("utility describes the image"));
    let utility = utility_model.clone().mediation("utility-vision-model", vision());

    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            Mediation {
                session: Some(&session),
                utility: Some(&utility),
            },
        )
        .await
        .expect("resolve completes");
    match &outcomes[0] {
        ProjectionOutcome::Resolved { producer, .. } => {
            assert_eq!(producer, "model:utility-vision-model:v1");
        }
        other => panic!("capable utility must project: {other:?}"),
    }
    assert_eq!(
        session_model.calls(),
        0,
        "MUST-NOT: model-mediated call without the capability check"
    );
    assert_eq!(utility_model.calls(), 1);
}

#[tokio::test]
async fn audio_transcript_mediation_gates_on_audio_capability() {
    let fix = fixture();
    let store = MediaStore::open(fix.dir.path()).expect("media store");
    let reference = store
        .store_media(b"RIFF----WAVEfmt fixture")
        .expect("store wav bytes");
    let blocks = vec![ContentBlock::Media(
        harness_core::event::MediaBlock::new(
            harness_core::event::MimeType::new("audio/wav").expect("mime"),
            reference,
            std::collections::BTreeMap::new(),
        )
        .expect("media block"),
    )];
    let model = ScriptedModel::new(Answer::Text("hello from the audio"));
    let audio_caps = DeliveryCapabilities {
        vision: false,
        audio_input: true,
    };
    let session = model.clone().mediation("audio-capable-model", audio_caps);

    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            Mediation {
                session: Some(&session),
                utility: None,
            },
        )
        .await
        .expect("resolve completes");
    match &outcomes[0] {
        ProjectionOutcome::Resolved { kind, producer, .. } => {
            assert_eq!(*kind, ProjectionKind::Transcript);
            assert_eq!(producer, "model:audio-capable-model:v1");
        }
        other => panic!("audio-capable model must transcribe: {other:?}"),
    }
    assert_eq!(model.calls(), 1);
}

#[tokio::test]
async fn no_capable_model_never_calls_any_model() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    let session_model = ScriptedModel::new(Answer::Text("never"));
    let utility_model = ScriptedModel::new(Answer::Text("never"));
    let session = session_model.clone().mediation("s", text_only());
    let utility = utility_model.clone().mediation("u", text_only());

    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            Mediation {
                session: Some(&session),
                utility: Some(&utility),
            },
        )
        .await
        .expect("resolve completes");
    let outcome = &outcomes[0];
    match outcome {
        ProjectionOutcome::Unprojectable { reason, .. } => {
            assert!(matches!(reason, Unprojectable::NoProjector));
            assert!(
                outcome.notice().is_some(),
                "the typed notice must name what is missing"
            );
        }
        other => panic!("no capable model must be unprojectable: {other:?}"),
    }
    assert_eq!(session_model.calls(), 0, "capability gate must precede any call");
    assert_eq!(utility_model.calls(), 0);
}

#[tokio::test]
async fn empty_or_tool_call_answers_are_typed_never_fabricated() {
    for answer in [Answer::Empty, Answer::ToolCall] {
        let fix = fixture();
        let blocks = image_blocks(fix.dir.path());
        let session = ScriptedModel::new(answer).mediation("m", vision());
        let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
            .resolve(
                Submission {
                    blocks: &blocks,
                    caps: text_only(),
                },
                Mediation {
                    session: Some(&session),
                    utility: None,
                },
            )
            .await
            .expect("resolve completes");
        match &outcomes[0] {
            ProjectionOutcome::Unprojectable { reason, .. } => {
                assert!(
                    matches!(reason, Unprojectable::ModelMediation(_)),
                    "non-text turns must be typed failures: {reason:?}"
                );
            }
            other => panic!("no fabrication from empty answers: {other:?}"),
        }
        let derived = DerivedStore::open(fix.dir.path()).expect("derived store");
        let source = harness_core::event::ContentHash::from_bytes(b"\x89PNG\r\n\x1a\nfixture-payload");
        assert_eq!(
            derived_cache(&derived, &source, DELIVERY_CACHE_PRODUCER, ProjectionKind::SceneDescription),
            None
        );
    }
}

#[tokio::test]
async fn hung_model_is_a_typed_timeout_and_the_marker_survives() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    let model = ScriptedModel::new(Answer::Hang);
    let mut session = model.clone().mediation("hung-model", vision());
    session.timeout = Duration::from_millis(150);

    let started = Instant::now();
    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            Mediation {
                session: Some(&session),
                utility: None,
            },
        )
        .await
        .expect("resolve itself must not hang");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the timeout must bound the hang"
    );
    match &outcomes[0] {
        ProjectionOutcome::Unprojectable { reason, .. } => {
            assert!(matches!(reason, Unprojectable::Timeout), "{reason:?}");
        }
        other => panic!("a hung model must be a typed timeout: {other:?}"),
    }
    let derived = DerivedStore::open(fix.dir.path()).expect("derived store");
    let source = harness_core::event::ContentHash::from_bytes(b"\x89PNG\r\n\x1a\nfixture-payload");
    assert_eq!(
        derived_cache(&derived, &source, DELIVERY_CACHE_PRODUCER, ProjectionKind::SceneDescription),
        None,
        "nothing stored; the block keeps its marker"
    );
}

#[tokio::test]
async fn cache_hit_wins_over_model_drift_same_media_never_projects_twice() {
    let fix = fixture();
    let blocks = image_blocks(fix.dir.path());
    let first = ScriptedModel::new(Answer::Text("the first description"));
    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            Mediation {
                session: Some(&first.clone().mediation("model-a", vision())),
                utility: None,
            },
        )
        .await
        .expect("first resolve");
    assert!(resolved(&outcomes[0]));

    let second_model = ScriptedModel::new(Answer::Text("a DIFFERENT description"));
    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            Mediation {
                session: Some(&second_model.clone().mediation("model-b", vision())),
                utility: None,
            },
        )
        .await
        .expect("second resolve");
    assert!(resolved(&outcomes[0]), "cache hit must resolve: {outcomes:?}");
    assert_eq!(second_model.calls(), 0, "same media never projects twice");
    let derived = DerivedStore::open(fix.dir.path()).expect("derived store");
    let source = harness_core::event::ContentHash::from_bytes(b"\x89PNG\r\n\x1a\nfixture-payload");
    assert_eq!(
        derived_cache(&derived, &source, DELIVERY_CACHE_PRODUCER, ProjectionKind::SceneDescription),
        Some(b"the first description".to_vec()),
        "stale-state pin: the cached projection is byte-stable"
    );
}

#[tokio::test]
async fn missing_source_bytes_are_typed_not_guessed() {
    let fix = fixture();
    // A block referencing bytes that were never stored.
    let phantom = harness_core::event::MediaRef {
        hash: harness_core::event::ContentHash::from_bytes(b"never stored anywhere"),
        bytes: 21,
    };
    let blocks = vec![ContentBlock::Image(
        ImageBlock::new("image/png", 4, 4, phantom).expect("block"),
    )];
    let outcomes = ProjectionResolver::new(fix.dir.path(), &fix.registry)
        .resolve(
            Submission {
                blocks: &blocks,
                caps: text_only(),
            },
            no_mediation(),
        )
        .await
        .expect("resolve completes");
    match &outcomes[0] {
        ProjectionOutcome::Unprojectable { reason, .. } => {
            assert!(matches!(reason, Unprojectable::MissingSource), "{reason:?}");
        }
        other => panic!("missing bytes must be typed: {other:?}"),
    }
}

#[test]
fn delivery_cache_producer_matches_the_providers_delivery_alias() {
    // The M8 delivery cache (harness-providers MediaDelivery) inlines
    // under exactly this producer id; the equality is a cross-crate
    // contract, pinned here textually.
    assert_eq!(DELIVERY_CACHE_PRODUCER, "delivery");
}

#[tokio::test]
async fn resolve_reports_io_error_when_the_session_dir_is_unopenable() {
    let registry = ProjectionRegistry::new();
    // A FILE where the session dir should be: open must fail typed io.
    let file = tempfile::NamedTempFile::new().expect("temp file");
    let outcome: io::Result<Vec<ProjectionOutcome>> = ProjectionResolver::new(file.path(), &registry)
        .resolve(
            Submission {
                blocks: &[],
                caps: text_only(),
            },
            no_mediation(),
        )
        .await;
    assert!(outcome.is_err(), "unopenable session dir must be io-typed");
}
