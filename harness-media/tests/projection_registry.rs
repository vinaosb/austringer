//! M10: the projection seam — `ProjectionProvider` trait, registry
//! registration, and the typed `NoProjector` posture (no format adapters
//! exist yet; the registry honestly refuses until M11+ registers
//! deterministic projectors alongside their adapters).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use harness_core::event::ProjectionKind;
use harness_media::error::MediaError;
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::projection::{
    ProjectionContext, ProjectionProvider, ProjectionRegistry, ProjectionWiringError,
};

/// Counting deterministic projector: returns fixed text, counts calls
/// (the zero-recompute counter-pin rides this).
struct CountingProjector {
    kind: ProjectionKind,
    calls: AtomicUsize,
    text: &'static str,
}

impl ProjectionProvider for CountingProjector {
    fn kind(&self) -> ProjectionKind {
        self.kind
    }

    fn project(&self, _bytes: &[u8], _ctx: &ProjectionContext<'_>) -> Result<String, MediaError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.text.to_owned())
    }
}

/// Failing projector: always the typed error it was built with (the
/// corrupt-media adversarial arm).
struct FailingProjector {
    kind: ProjectionKind,
    failure: MediaError,
}

impl ProjectionProvider for FailingProjector {
    fn kind(&self) -> ProjectionKind {
        self.kind
    }

    fn project(&self, _bytes: &[u8], _ctx: &ProjectionContext<'_>) -> Result<String, MediaError> {
        Err(self.failure.clone())
    }
}

fn context() -> ProjectionContext<'static> {
    ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    }
}

#[test]
fn empty_registry_answers_typed_no_projector_never_a_guess() {
    let registry = ProjectionRegistry::new();
    let outcome = registry.project(&FormatId::Png, ProjectionKind::SceneDescription, b"\x89PNG\r\n\x1a\n", &context());
    assert_eq!(outcome, Err(MediaError::NoProjector));
}

#[test]
fn registered_projector_dispatches_and_counts() {
    let mut registry = ProjectionRegistry::new();
    let projector = Arc::new(CountingProjector {
        kind: ProjectionKind::SceneDescription,
        calls: AtomicUsize::new(0),
        text: "png: 8x8 rgba",
    });
    registry
        .register(FormatId::Png, Arc::clone(&projector) as Arc<dyn ProjectionProvider>)
        .expect("first registration must succeed");
    let projected = registry
        .project(&FormatId::Png, ProjectionKind::SceneDescription, b"\x89PNG\r\n\x1a\n", &context())
        .expect("registered projector must dispatch");
    assert_eq!(projected, "png: 8x8 rgba");
    assert_eq!(projector.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn duplicate_registration_is_refused_typed_and_original_survives() {
    let mut registry = ProjectionRegistry::new();
    let first = Arc::new(CountingProjector {
        kind: ProjectionKind::TextProjection,
        calls: AtomicUsize::new(0),
        text: "first",
    });
    registry
        .register(FormatId::Svg, Arc::clone(&first) as Arc<dyn ProjectionProvider>)
        .expect("first registration must succeed");
    let second = Arc::new(CountingProjector {
        kind: ProjectionKind::TextProjection,
        calls: AtomicUsize::new(0),
        text: "second",
    });
    let refusal = registry
        .register(FormatId::Svg, second as Arc<dyn ProjectionProvider>)
        .expect_err("one format, one projector");
    assert!(
        matches!(refusal, ProjectionWiringError::DuplicateProjector { format } if format == FormatId::Svg),
        "duplicate must be typed: {refusal:?}"
    );
    let projected = registry
        .project(&FormatId::Svg, ProjectionKind::TextProjection, b"<svg/>", &context())
        .expect("the original projector must keep answering");
    assert_eq!(projected, "first");
}

#[test]
fn kind_mismatch_is_typed_no_projector() {
    let mut registry = ProjectionRegistry::new();
    registry
        .register(
            FormatId::Wav,
            Arc::new(CountingProjector {
                kind: ProjectionKind::TextProjection,
                calls: AtomicUsize::new(0),
                text: "audio: wav",
            }) as Arc<dyn ProjectionProvider>,
        )
        .expect("registration must succeed");
    let outcome = registry.project(&FormatId::Wav, ProjectionKind::Transcript, b"RIFF", &context());
    assert_eq!(outcome, Err(MediaError::NoProjector));
}

#[test]
fn provider_failure_propagates_typed_never_fabricated() {
    let mut registry = ProjectionRegistry::new();
    registry
        .register(
            FormatId::Png,
            Arc::new(FailingProjector {
                kind: ProjectionKind::SceneDescription,
                failure: MediaError::ParseFailure,
            }) as Arc<dyn ProjectionProvider>,
        )
        .expect("registration must succeed");
    let outcome = registry.project(&FormatId::Png, ProjectionKind::SceneDescription, b"corrupt", &context());
    assert_eq!(outcome, Err(MediaError::ParseFailure));
}
