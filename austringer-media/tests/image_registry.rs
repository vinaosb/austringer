//! M28 image-family registration tests: the M28 pattern mirrored from
//! M11/M12/M13 - descriptor claims flip to Supported synchronized with
//! pure handler bindings, every dispatched artifact carries producer
//! disclosure, gif keeps its vision-only honesty row, and the M10
//! projection seam carries the per-format deterministic projectors.

use std::sync::Arc;

use austringer_core::event::ProjectionKind;
use austringer_media::adapter::image;
use austringer_media::descriptor::{OpSupport, Operation};
use austringer_media::error::MediaError;
use austringer_media::executor::{OpOutput, OpRequest};
use austringer_media::format::FormatId;
use austringer_media::projection::{ProjectionContext, ProjectionRegistry};

const PNG_BYTES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/png.png"));

fn registry() -> austringer_media::executor::ExecutorRegistry {
    austringer_media::executor::registry()
}

fn assert_all_ops_supported(format: FormatId) {
    let registry = registry();
    let entry = registry.lookup(&format).expect("entry registered");
    for op in [
        Operation::Probe,
        Operation::Extract,
        Operation::Generate,
        Operation::Verify,
    ] {
        assert!(
            matches!(entry.descriptor.ops.get(&op), Some(OpSupport::Supported)),
            "{format} {op} must be claimed Supported"
        );
    }
    assert_eq!(entry.tier, austringer_media::executor::AdapterTier::Pure);
}

#[test]
fn image_family_descriptors_claim_supported_with_pure_tier() {
    assert_all_ops_supported(FormatId::Png);
    assert_all_ops_supported(FormatId::Jpeg);
    assert_all_ops_supported(FormatId::Webp);
    assert_all_ops_supported(FormatId::Avif);
}

#[test]
fn jxl_descriptor_claims_extract_only() {
    let registry = registry();
    let entry = registry.lookup(&FormatId::Jxl).expect("jxl registered");
    assert!(matches!(
        entry.descriptor.ops.get(&Operation::Extract),
        Some(OpSupport::Supported)
    ));
    for op in [Operation::Generate, Operation::Verify] {
        match entry.descriptor.ops.get(&op) {
            Some(OpSupport::Unsupported { reason }) => assert!(
                reason.to_lowercase().contains("adoption")
                    || reason.to_lowercase().contains("extract-only"),
                "jxl {op} refusal is the honesty row: {reason}"
            ),
            other => panic!("jxl {op} must stay Unsupported, got {other:?}"),
        }
    }
}

#[test]
fn png_extract_dispatch_carries_image_producer_disclosure() {
    let produced = registry()
        .dispatch(
            FormatId::Png,
            OpRequest::Extract {
                bytes: PNG_BYTES,
                budgets: Default::default(),
            },
        )
        .expect("png extract dispatch");
    let OpOutput::Extract(extracted) = produced.inner() else {
        panic!("expected Extract output");
    };
    assert_eq!(extracted.tree["width"], serde_json::json!(12));
    let producer = produced.producer();
    assert_eq!(producer.tier, austringer_media::executor::AdapterTier::Pure);
    assert_eq!(producer.engine, "austringer-media::image::png");
    assert_eq!(producer.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(producer.command, None);
}

#[test]
fn gif_honesty_row_survives_the_image_family_landing() {
    // gif: probe + native vision delivery ONLY - extract is the PERMANENT
    // typed refusal (M28 lands image adapters AROUND gif, never into it).
    let error = registry()
        .dispatch(
            FormatId::Gif,
            OpRequest::Extract {
                bytes: b"GIF89a",
                budgets: Default::default(),
            },
        )
        .expect_err("gif extract must refuse");
    match error {
        MediaError::UnsupportedOperation { format, op, reason } => {
            assert_eq!(format, FormatId::Gif);
            assert_eq!(op, Operation::Extract);
            assert!(
                reason.to_lowercase().contains("vision"),
                "gif refusal names the vision-only lane: {reason}"
            );
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn deterministic_projectors_register_for_every_image_format() {
    for (format, engine_bytes) in [
        (FormatId::Png, PNG_BYTES),
        (
            FormatId::Jpeg,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/jpeg.jpg")),
        ),
        (
            FormatId::Webp,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/webp.webp")),
        ),
        (
            FormatId::Avif,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/avif.avif")),
        ),
        (
            FormatId::Jxl,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/jxl.jxl")),
        ),
    ] {
        let mut projections = ProjectionRegistry::new();
        projections
            .register(format, Arc::new(image::projector(format)))
            .expect("first registration wins");
        let context = ProjectionContext {
            budgets: Default::default(),
            name_hint: None,
        };
        let projected = projections
            .project(
                &format,
                ProjectionKind::TextProjection,
                engine_bytes,
                &context,
            )
            .unwrap_or_else(|error| panic!("{format} projects: {error:?}"));
        assert!(
            !projected.is_empty(),
            "{format}: no fabricated-empty projections"
        );
        // A kind mismatch is the typed NoProjector (one projector, one
        // kind - the M10 seam contract).
        let mismatch = projections.project(
            &format,
            ProjectionKind::SceneDescription,
            engine_bytes,
            &context,
        );
        assert!(
            matches!(mismatch, Err(MediaError::NoProjector)),
            "{format}: got {mismatch:?}"
        );
    }
}
