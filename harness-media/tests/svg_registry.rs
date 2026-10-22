//! M11 svg registration-pattern tests: the adapter is wired into the M6
//! executor (descriptor claims Supported, pure handlers bound, producer
//! disclosure on every artifact) and the M10 projection seam carries the
//! svg deterministic projector. This file is the template every later
//! adapter todo (M12-M23, M28-M30) mirrors.

use std::sync::Arc;

use harness_core::event::ProjectionKind;
use harness_media::adapter::svg;
use harness_media::error::MediaError;
use harness_media::executor::{OpOutput, OpRequest};
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use harness_media::projection::{ProjectionContext, ProjectionRegistry};
use serde_json::json;

const MINIMAL_SVG: &[u8] =
    b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 4 4\"><rect width=\"1\" height=\"1\"/></svg>";

fn registry() -> harness_media::executor::ExecutorRegistry {
    harness_media::executor::registry()
}

#[test]
fn descriptor_claims_extract_generate_verify_supported() {
    // Given: the day-one registry with the svg adapter wired (M11).
    let registry = registry();
    let entry = registry.lookup(&FormatId::Svg).expect("svg registered");
    // Then: probe + extract + generate + verify are all Supported claims
    // synchronized with bound pure handlers (the builder enforced it).
    for op in [
        harness_media::descriptor::Operation::Probe,
        harness_media::descriptor::Operation::Extract,
        harness_media::descriptor::Operation::Generate,
        harness_media::descriptor::Operation::Verify,
    ] {
        assert!(
            matches!(
                entry.descriptor.ops.get(&op),
                Some(harness_media::descriptor::OpSupport::Supported)
            ),
            "{op} must be claimed Supported"
        );
    }
    // And: the tier is pure (in-process quick-xml/usvg, no tools).
    assert_eq!(entry.tier, harness_media::executor::AdapterTier::Pure);
}

#[test]
fn extract_dispatch_carries_svg_producer_disclosure() {
    // Given: valid svg bytes.
    // When: extract is dispatched through the executor.
    let produced = registry()
        .dispatch(
            FormatId::Svg,
            OpRequest::Extract {
                bytes: MINIMAL_SVG,
                budgets: Default::default(),
            },
        )
        .expect("extract dispatch");
    // Then: the artifact is the canonical tree + bounded projection...
    let OpOutput::Extract(extracted) = produced.inner() else {
        panic!("expected Extract output");
    };
    assert_eq!(extracted.tree["census"]["rect"], json!(1));
    // ...and the producer disclosure names the svg engine, pure tier,
    // crate version, no command (nothing external ran).
    let producer = produced.producer();
    assert_eq!(producer.tier, harness_media::executor::AdapterTier::Pure);
    assert_eq!(producer.engine, "harness-media::svg");
    assert_eq!(producer.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(producer.command, None);
}

#[test]
fn generate_and_verify_dispatch_round_trip_live() {
    // Given: a spec dispatched through the executor.
    let spec = GenerationSpec {
        format: FormatId::Svg,
        params: json!({"viewBox": [0.0, 0.0, 8.0, 8.0],
                       "shapes": [{"type": "circle", "cx": 4.0, "cy": 4.0, "r": 2.0}]}),
    };
    let expected = svg::SvgSpec::from_generation(&spec)
        .expect("spec parses")
        .canonical_tree();
    let registry = registry();
    let generated = registry
        .dispatch(FormatId::Svg, OpRequest::Generate { spec: &spec })
        .expect("generate dispatch");
    let OpOutput::Generate(bytes) = generated.inner() else {
        panic!("expected Generate output");
    };
    assert_eq!(generated.producer().engine, "harness-media::svg");
    // When: verified through the executor too.
    let verified = registry
        .dispatch(
            FormatId::Svg,
            OpRequest::Verify {
                expected: &expected,
                bytes,
            },
        )
        .expect("verify dispatch");
    // Then: PASS with svg-engine disclosure.
    let OpOutput::Verify(verdict) = verified.inner() else {
        panic!("expected Verify output");
    };
    assert!(verdict.is_pass());
    assert_eq!(verified.producer().engine, "harness-media::svg");
}

#[test]
fn xxe_through_the_executor_is_a_typed_dispatch_error() {
    // Given: the committed XXE fixture.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("svg-xxe.svg");
    let bytes = std::fs::read(&path).expect("fixture present");
    // When: extracted through dispatch.
    let error = registry()
        .dispatch(
            FormatId::Svg,
            OpRequest::Extract {
                bytes: &bytes,
                budgets: Default::default(),
            },
        )
        .expect_err("XXE must be refused");
    // Then: the typed ParseFailure propagates through dispatch - never a
    // panic, never a partial artifact.
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn deterministic_projector_registers_and_matches_extract_projection() {
    // Given: the svg projector registered in an M10 projection registry.
    let mut projections = ProjectionRegistry::new();
    projections
        .register(FormatId::Svg, Arc::new(svg::SvgProjector))
        .expect("first registration wins");
    let context = ProjectionContext {
        budgets: Default::default(),
        name_hint: None,
    };
    // When: projected for the TextProjection kind.
    let projected = projections
        .project(
            &FormatId::Svg,
            ProjectionKind::TextProjection,
            MINIMAL_SVG,
            &context,
        )
        .expect("svg projects");
    // Then: it is exactly extract's text_projection (a thin wrap, by
    // design - never a fabricated transcript).
    let extracted = svg::extract(MINIMAL_SVG, Default::default()).expect("extract");
    assert_eq!(projected, extracted.text_projection);
    // And: a kind mismatch is the typed NoProjector (one projector, one
    // kind - the M10 seam contract).
    let mismatch = projections.project(
        &FormatId::Svg,
        ProjectionKind::SceneDescription,
        MINIMAL_SVG,
        &context,
    );
    assert!(
        matches!(mismatch, Err(MediaError::NoProjector)),
        "got {mismatch:?}"
    );
}

#[test]
fn duplicate_projector_registration_is_refused() {
    // Given: a registry with the svg projector already registered.
    let mut projections = ProjectionRegistry::new();
    projections
        .register(FormatId::Svg, Arc::new(svg::SvgProjector))
        .expect("first registration");
    // When: a second svg projector registers.
    let error = projections
        .register(FormatId::Svg, Arc::new(svg::SvgProjector))
        .expect_err("one format, one projector");
    // Then: typed duplicate refusal, original intact.
    assert_eq!(
        error,
        harness_media::projection::ProjectionWiringError::DuplicateProjector {
            format: FormatId::Svg
        }
    );
}

#[test]
fn thumbnails_are_honestly_absent_in_v02() {
    // The honesty row: rasterization is explicitly out of scope for the
    // svg adapter in v0.2 - extraction returns NO thumbnail references
    // (no fake placeholders, no deferred-yet-present stubs).
    let extracted = svg::extract(MINIMAL_SVG, Default::default()).expect("extract");
    assert!(extracted.thumbnails.is_empty());
}
