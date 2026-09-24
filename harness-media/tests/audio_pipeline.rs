//! M14 audio transcription seam: transcripts are MODEL-MEDIATED ONLY
//! (M10 pipeline) - the audio adapter registers a deterministic
//! TextProjection projector (the one-line summary), never a Transcript
//! projector; with no capable model and no external tool (whisper.cpp
//! lands with the M15 manifest lane), a text-only session's queued
//! Transcript resolves to the typed NoProjector outcome plus the system
//! notice, and nothing is ever fabricated. Hermetic.

use harness_core::event::media::MediaStore;
use harness_core::event::{ContentBlock, MediaBlock, ProjectionKind};
use harness_core::media_delivery::DeliveryCapabilities;
use harness_media::adapter::audio;
use harness_media::format::FormatId;
use harness_media::pipeline::{
    Mediation, ProjectionOutcome, ProjectionResolver, Submission, Unprojectable,
};
use harness_media::projection::{ProjectionContext, ProjectionRegistry};

const WAV: &[u8] = include_bytes!("../fixtures/audio/sine-440-16k-mono.wav");

fn audio_blocks(dir: &std::path::Path) -> Vec<ContentBlock> {
    let store = MediaStore::open(dir).expect("media store");
    let reference = store.store_media(WAV).expect("store wav bytes");
    let mime = harness_core::event::MimeType::new("audio/wav").expect("audio/wav mime");
    let block = MediaBlock::new(mime, reference, std::collections::BTreeMap::new())
        .expect("media block");
    vec![ContentBlock::Media(block)]
}

#[tokio::test]
async fn deterministic_text_projection_registers_for_the_audio_family() {
    // Given: the audio projector registered alongside the adapter, for
    // every pure-decode format of the family.
    let mut registry = ProjectionRegistry::new();
    audio::register_projections(&mut registry).expect("register audio projectors");
    let context = ProjectionContext {
        budgets: harness_media::extract::ExtractBudgets::default(),
        name_hint: None,
    };
    for format in [FormatId::Wav, FormatId::Flac, FormatId::Mp3, FormatId::Ogg] {
        // When: a TextProjection is requested directly.
        let projected = registry
            .project(&format, ProjectionKind::TextProjection, WAV, &context)
            .unwrap_or_else(|error| panic!("{format} must project: {error}"));
        // Then: the deterministic one-line summary.
        assert_eq!(projected, "audio: pcm_s16le 16000Hz 1ch 0:00");
    }
    // And: opus never registers (its decode is the M17 ffmpeg lane).
    assert!(matches!(
        registry.project(&FormatId::Opus, ProjectionKind::TextProjection, WAV, &context),
        Err(harness_media::error::MediaError::NoProjector)
    ));
    // And: a second registration attempt is the typed wiring error.
    assert!(
        audio::register_projections(&mut registry).is_err(),
        "one format, one projector"
    );
}

#[tokio::test]
async fn transcript_without_capable_model_or_tool_is_typed_no_projector_with_notice() {
    // Given: an audio/wav block submitted by a text-only model, the
    // audio projector registered, and NO mediation candidates (no
    // capable model; whisper.cpp is the M15 manifest lane and is not
    // installed).
    let dir = tempfile::tempdir().expect("temp session dir");
    let blocks = audio_blocks(dir.path());
    let mut registry = ProjectionRegistry::new();
    audio::register_projections(&mut registry).expect("register audio projectors");
    let resolver = ProjectionResolver::new(dir.path(), &registry);
    // When: resolved (the planner queues a Transcript for audio to a
    // text-only session).
    let outcomes = resolver
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
        .expect("resolve pass");
    // Then: exactly one queued projection, unprojectable for the typed
    // reason - never a fabricated transcript.
    assert_eq!(outcomes.len(), 1, "{outcomes:?}");
    match &outcomes[0] {
        ProjectionOutcome::Unprojectable {
            kind,
            reason: Unprojectable::NoProjector,
            ..
        } => {
            assert_eq!(*kind, ProjectionKind::Transcript);
            // And: the system notice names what is missing.
            let notice = outcomes[0]
                .notice()
                .expect("unprojectable carries a notice");
            assert!(
                notice.contains("transcript"),
                "notice names the missing transcript: {notice}"
            );
        }
        other => panic!("expected typed NoProjector, got {other:?}"),
    }
    // And: the derived store holds NO transcript artifact for the source
    // (the marker-only posture is real, not just reported).
    let derived = harness_core::event::derived::DerivedStore::open(dir.path()).expect("derived");
    let source = harness_core::event::ContentHash::from_bytes(WAV);
    assert!(
        derived
            .load_derived(&source, "delivery", ProjectionKind::Transcript)
            .expect("store read")
            .is_none(),
        "no transcript may be fabricated or cached"
    );
}

/// The projector's kind is TextProjection and ONLY TextProjection: this
/// pins the model-mediated transcript boundary at the type level (a
/// Transcript projector from the audio adapter would be a fabrication
/// seam).
#[test]
fn audio_projector_kind_is_text_projection_only() {
    let mut registry = ProjectionRegistry::new();
    audio::register_projections(&mut registry).expect("register audio projectors");
    let context = ProjectionContext {
        budgets: harness_media::extract::ExtractBudgets::default(),
        name_hint: None,
    };
    assert!(matches!(
        registry.project(&FormatId::Wav, ProjectionKind::Transcript, WAV, &context),
        Err(harness_media::error::MediaError::NoProjector)
    ));
}
