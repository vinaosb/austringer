//! M6 executor dispatch tests: tiered dispatch, producer disclosure on
//! every artifact, subprocess-tier typed unavailability (pre-M15), and
//! dispatch repeatability. Hermetic: no network, no filesystem writes, no
//! external tools.

use austringer_media::error::MediaError;
use austringer_media::executor::{OpRequest, Produced};
use austringer_media::format::FormatId;
use austringer_media::generate::GenerationSpec;

/// The PNG magic (probe lane fixture).
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The PDF magic.
const PDF_MAGIC: &[u8] = b"%PDF-2.0\n";

/// A markdown leading-content lane fixture.
const MARKDOWN_LEADING: &[u8] = b"# heading\n\nbody text\n";

fn registry() -> austringer_media::executor::ExecutorRegistry {
    austringer_media::executor::registry()
}

fn dispatch_probe(
    registry: &austringer_media::executor::ExecutorRegistry,
    format: FormatId,
    bytes: &[u8],
) -> Result<Produced, MediaError> {
    registry.dispatch(
        format,
        OpRequest::Probe {
            bytes,
            name_hint: None,
        },
    )
}

#[test]
fn probe_dispatch_returns_produced_with_full_producer_disclosure() {
    // Given: the day-one registry and PNG magic bytes.
    // When: probe is dispatched.
    let produced =
        dispatch_probe(&registry(), FormatId::Png, PNG_MAGIC).expect("probe dispatch must succeed");
    // Then: the output identifies png with Certain confidence...
    match produced.inner() {
        austringer_media::executor::OpOutput::Probe(result) => {
            let austringer_media::probe::ProbeResult::Identified {
                format, confidence, ..
            } = result
            else {
                panic!("png magic must be Identified, got {result:?}");
            };
            assert_eq!(*format, FormatId::Png);
            assert_eq!(*confidence, austringer_media::probe::Confidence::Certain);
        }
        other => panic!("expected probe output, got {other:?}"),
    }
    // ...and the producer disclosure is complete: pure tier, named engine,
    // crate version, no command (nothing external ran).
    let producer = produced.producer();
    assert_eq!(producer.tier, austringer_media::executor::AdapterTier::Pure);
    assert_eq!(producer.engine, "austringer-media::probe");
    assert_eq!(producer.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(producer.command, None);
}

#[test]
fn dispatch_is_repeatable_and_leaves_no_stale_state() {
    // Given: one registry, two identical probe dispatches.
    let registry = registry();
    let first = dispatch_probe(&registry, FormatId::Png, PNG_MAGIC);
    let second = dispatch_probe(&registry, FormatId::Png, PNG_MAGIC);
    // Then: both succeed with identical disclosures (dispatch never
    // mutates registry state between calls).
    let (Ok(first), Ok(second)) = (first, second) else {
        panic!("both dispatches must succeed");
    };
    assert_eq!(first.producer(), second.producer());
}

#[test]
fn generate_on_unwired_pure_format_is_typed_refusal_citing_the_lane() {
    // Given: xlsx generation arrives with M19 (not yet landed; this pin
    // moved off svg at M11, off gltf at M12, off pdf at M13, off wav
    // at M14, and off the M18 read citation at M18 - the office WRITE
    // lane, M19, is the next still-pending pure generate lane).
    let spec = GenerationSpec {
        format: FormatId::Xlsx,
        params: serde_json::json!({"tones": []}),
    };
    // When: generate is dispatched.
    let error = registry()
        .dispatch(FormatId::Xlsx, OpRequest::Generate { spec: &spec })
        .expect_err("generate must refuse until M19 lands");
    // Then: typed UnsupportedOperation naming format, op, and the lane.
    match error {
        MediaError::UnsupportedOperation { format, op, reason } => {
            assert_eq!(format, FormatId::Xlsx);
            assert_eq!(op, austringer_media::descriptor::Operation::Generate);
            assert!(reason.contains("M19"), "reason cites the lane: {reason}");
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn mp3_generate_is_an_encode_redirect_to_the_ffmpeg_lane() {
    let spec = GenerationSpec {
        format: FormatId::Mp3,
        params: serde_json::json!({}),
    };
    let error = registry()
        .dispatch(FormatId::Mp3, OpRequest::Generate { spec: &spec })
        .expect_err("mp3 generate must refuse");
    match error {
        MediaError::UnsupportedOperation { format, op, reason } => {
            assert_eq!(format, FormatId::Mp3);
            assert_eq!(op, austringer_media::descriptor::Operation::Generate);
            assert!(
                reason.contains("ffmpeg"),
                "encode-redirect names the subprocess lane: {reason}"
            );
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn jxl_verify_on_extract_only_lane_is_typed_unsupported_operation() {
    // Given: jxl is extract-only (generation gated on the adoption
    // trigger) - there is no writer whose round trip verify could gate.
    // When: verify is dispatched.
    let expected = serde_json::json!({});
    let error = registry()
        .dispatch(
            FormatId::Jxl,
            OpRequest::Verify {
                expected: &expected,
                bytes: b"",
            },
        )
        .expect_err("verify must refuse on an extract-only format");
    // Then: typed UnsupportedOperation, never a silent pass.
    match error {
        MediaError::UnsupportedOperation { format, op, reason } => {
            assert_eq!(format, FormatId::Jxl);
            assert_eq!(op, austringer_media::descriptor::Operation::Verify);
            assert!(
                reason.to_lowercase().contains("extract-only"),
                "reason names the extract-only honesty: {reason}"
            );
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn subprocess_dispatch_routes_through_the_m15_tool_executor() {
    // Given: the decided subprocess lanes (pandoc for pptx generation,
    // blender for blend) with the default all-tools-disabled executor.
    let spec = GenerationSpec {
        format: FormatId::Pptx,
        params: serde_json::json!({}),
    };
    let pptx = registry()
        .dispatch(FormatId::Pptx, OpRequest::Generate { spec: &spec })
        .expect_err("pptx generate must be typed-unavailable");
    match pptx {
        MediaError::ToolUnavailable { tool, install_hint } => {
            assert_eq!(tool, "pandoc");
            assert!(!install_hint.is_empty(), "install hint is actionable");
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }
    let blend = registry()
        .dispatch(
            FormatId::Blend,
            OpRequest::Extract {
                bytes: b"BLENDER_v305",
                budgets: Default::default(),
            },
        )
        .expect_err("blend extract must be typed-unavailable");
    match blend {
        MediaError::ToolUnavailable { tool, install_hint } => {
            assert_eq!(tool, "blender");
            assert!(!install_hint.is_empty());
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }
}

#[test]
fn probe_disclosure_is_populated_across_every_probe_lane() {
    // Given: binary-magic, document-magic, and text-heuristic lanes.
    let registry = registry();
    for (format, bytes) in [
        (FormatId::Png, PNG_MAGIC),
        (FormatId::Pdf, PDF_MAGIC),
        (FormatId::Markdown, MARKDOWN_LEADING),
    ] {
        // When: probed.
        let Ok(produced) = dispatch_probe(&registry, format, bytes) else {
            panic!("{format} probe must succeed");
        };
        // Then: the disclosure fields are all populated (a Produced with
        // an empty engine/version is a disclosure hole).
        let producer = produced.producer();
        assert!(!producer.engine.is_empty(), "{format}: empty engine");
        assert!(!producer.version.is_empty(), "{format}: empty version");
        assert_eq!(
            producer.command, None,
            "{format}: pure probe has no command"
        );
    }
}
