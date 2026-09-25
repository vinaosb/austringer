//! M28 image-family generation tests: spec validation (typed
//! InvalidSpec), deterministic writers (byte-identical repeats), the
//! lossy/animated WebP redirect to the M17 ffmpeg lane, the JXL
//! adoption-gated generation refusal, and AVIF encode (tiny, fast speed,
//! own timeout budget). Hermetic throughout.

use harness_media::adapter::image;
use harness_media::error::MediaError;
use harness_media::executor::{OpOutput, OpRequest};
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::json;

fn spec(format: FormatId, params: serde_json::Value) -> GenerationSpec {
    GenerationSpec { format, params }
}

fn assert_invalid_spec(format: FormatId, params: serde_json::Value, label: &str) {
    let request = spec(format, params.clone());
    let parsed = image::ImageSpec::parse(&request, format).unwrap_or_else(|error| {
        panic!("{label}: parse unexpectedly failed for {params}: {error:?}")
    });
    let error = parsed
        .check(format)
        .err()
        .unwrap_or_else(|| panic!("{label}: expected InvalidSpec for {params}"));
    assert!(
        matches!(error, MediaError::InvalidSpec),
        "{label}: got {error:?}"
    );
}

fn dispatch_generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let produced =
        harness_media::executor::registry().dispatch(spec.format, OpRequest::Generate { spec })?;
    match produced.inner() {
        OpOutput::Generate(bytes) => Ok(bytes.clone()),
        other => panic!("expected Generate output, got {other:?}"),
    }
}

#[test]
fn png_generate_is_byte_deterministic_across_runs() {
    let request = spec(
        FormatId::Png,
        json!({
            "width": 24, "height": 16, "bit_depth": 16, "color": "gray_alpha",
            "pattern": {"kind": "checker", "color_a": [0, 0], "color_b": [255, 255], "cell": 4},
        }),
    );
    let first = image::png::generate(&request).expect("first generate");
    let second = image::png::generate(&request).expect("second generate");
    assert_eq!(
        first, second,
        "png writer is byte-deterministic (M1 contract)"
    );
    assert!(first.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn apng_generate_writes_declared_frame_count() {
    // Given: an animated spec (3 frames, 10 Hz).
    let request = spec(
        FormatId::Png,
        json!({
            "width": 8, "height": 8,
            "pattern": {"kind": "stripes", "color_a": [255, 0, 0], "color_b": [0, 0, 255], "cell": 2},
            "animated": {"num_frames": 3, "num_plays": 0, "delay_num": 1, "delay_den": 10},
        }),
    );
    // When: generated and extracted back.
    let bytes = image::png::generate(&request).expect("apng generates");
    let extracted = image::png::extract(&bytes, Default::default()).expect("apng extracts");
    // Then: the animation control carries the spec's counts.
    assert_eq!(
        extracted.tree["animated"],
        json!({"num_frames": 3, "num_plays": 0})
    );
}

#[test]
fn jpeg_generate_respects_quality_and_progressive_spec() {
    let baseline = spec(
        FormatId::Jpeg,
        json!({
            "width": 16, "height": 12, "quality": 90, "progressive": false,
            "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
        }),
    );
    let progressive = spec(
        FormatId::Jpeg,
        json!({
            "width": 16, "height": 12, "quality": 90, "progressive": true,
            "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
        }),
    );
    let baseline_bytes = dispatch_generate(&baseline).expect("baseline jpeg");
    let progressive_bytes = dispatch_generate(&progressive).expect("progressive jpeg");
    assert_ne!(
        baseline_bytes, progressive_bytes,
        "progressive changes the bytes"
    );
    let progressive_tree =
        image::jpeg::extract(&progressive_bytes, Default::default()).expect("extract");
    assert_eq!(progressive_tree.tree["progressive"], json!(true));
    // Quality is honored: q30 is strictly smaller than q95 on the same
    // pattern.
    let mut low = baseline.params.clone();
    low["quality"] = json!(30);
    let mut high = baseline.params.clone();
    high["quality"] = json!(95);
    let low_bytes = dispatch_generate(&spec(FormatId::Jpeg, low)).expect("q30");
    let high_bytes = dispatch_generate(&spec(FormatId::Jpeg, high)).expect("q95");
    assert!(
        low_bytes.len() < high_bytes.len(),
        "q30 ({} bytes) < q95 ({} bytes)",
        low_bytes.len(),
        high_bytes.len()
    );
}

#[test]
fn jpeg_generate_is_byte_deterministic_across_runs() {
    let request = spec(
        FormatId::Jpeg,
        json!({
            "width": 16, "height": 12, "quality": 85,
            "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
        }),
    );
    assert_eq!(
        image::jpeg::generate(&request).expect("first"),
        image::jpeg::generate(&request).expect("second"),
        "jpeg writer is byte-deterministic (M1 contract)"
    );
}

#[test]
fn webp_lossless_generate_decodes_back() {
    let request = spec(
        FormatId::Webp,
        json!({
            "width": 12, "height": 8,
            "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
        }),
    );
    let bytes = dispatch_generate(&request).expect("webp lossless");
    assert!(bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP");
    let extracted = image::webp::extract(&bytes, Default::default()).expect("decodes back");
    assert_eq!(extracted.tree["lossless"], json!(true));
}

#[test]
fn lossy_webp_encode_request_redirects_to_the_ffmpeg_lane() {
    // The honesty row: lossy/animated WebP encode is the C libwebp path -
    // typed redirect to M17, never a fake pure-Rust claim.
    let request = spec(
        FormatId::Webp,
        json!({
            "width": 12, "height": 8, "lossy": true,
            "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
        }),
    );
    let error = dispatch_generate(&request).expect_err("lossy encode must refuse");
    match error {
        MediaError::UnsupportedOperation { format, op, reason } => {
            assert_eq!(format, FormatId::Webp);
            assert_eq!(op, harness_media::descriptor::Operation::Generate);
            assert!(
                reason.contains("ffmpeg"),
                "redirect names the lane: {reason}"
            );
            assert!(reason.contains("M17"), "redirect cites the todo: {reason}");
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn animated_webp_encode_request_redirects_to_the_ffmpeg_lane() {
    let request = spec(
        FormatId::Webp,
        json!({
            "width": 12, "height": 8,
            "pattern": {"kind": "solid", "color_a": [0, 0, 0]},
            "animated": {"num_frames": 2, "num_plays": 0, "delay_num": 1, "delay_den": 10},
        }),
    );
    let error = dispatch_generate(&request).expect_err("animated webp encode must refuse");
    match error {
        MediaError::UnsupportedOperation { reason, .. } => {
            assert!(
                reason.contains("ffmpeg"),
                "redirect names the lane: {reason}"
            );
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn avif_generate_encodes_and_probes_avif_brand() {
    // Tiny fixture, fastest speed: the ravif encode is SLOW by nature and
    // carries its own budgets.json timeout - excluded from the <=1s pure
    // round-trip line (plan M28).
    let request = spec(
        FormatId::Avif,
        json!({
            "width": 8, "height": 6, "quality": 60.0, "speed": 10,
            "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
        }),
    );
    let bytes = dispatch_generate(&request).expect("avif encodes");
    let result = harness_media::probe::probe(&bytes, None);
    match result {
        harness_media::probe::ProbeResult::Identified {
            format, confidence, ..
        } => {
            assert_eq!(format, FormatId::Avif);
            assert_eq!(confidence, harness_media::probe::Confidence::Certain);
        }
        other => panic!("encoded bytes must probe avif, got {other:?}"),
    }
}

#[test]
fn avif_generate_is_byte_deterministic_single_threaded() {
    let request = spec(
        FormatId::Avif,
        json!({
            "width": 8, "height": 6, "quality": 60.0, "speed": 10,
            "pattern": {"kind": "solid", "color_a": [80, 90, 100]},
        }),
    );
    let first = image::avif::generate(&request).expect("first avif");
    let second = image::avif::generate(&request).expect("second avif");
    assert_eq!(
        first, second,
        "single-threaded no-asm rav1e is deterministic"
    );
}

#[test]
fn jxl_generate_is_refused_gated_on_the_adoption_trigger() {
    let request = spec(
        FormatId::Jxl,
        json!({
            "width": 8, "height": 8,
            "pattern": {"kind": "solid", "color_a": [0, 0, 0]},
        }),
    );
    let error = dispatch_generate(&request).expect_err("jxl generation is gated");
    match error {
        MediaError::UnsupportedOperation { format, reason, .. } => {
            assert_eq!(format, FormatId::Jxl);
            assert!(
                reason.to_lowercase().contains("adoption"),
                "refusal cites the adoption trigger: {reason}"
            );
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn invalid_specs_are_typed_never_guessed() {
    for (format, params, label) in [
        (
            FormatId::Png,
            json!({"pattern": {"kind": "solid", "color_a": [0,0,0]}}),
            "missing height",
        ),
        (
            FormatId::Png,
            json!({"width": 0, "height": 8, "pattern": {"kind": "solid", "color_a": [0,0,0]}}),
            "zero width",
        ),
        (
            FormatId::Png,
            json!({"width": 8, "height": 8}),
            "missing pattern",
        ),
        (
            FormatId::Png,
            json!({"width": 8, "height": 8, "pattern": {"kind": "solid", "color_a": [0,0]}}),
            "color arity mismatch",
        ),
        (
            FormatId::Png,
            json!({"width": 8, "height": 8, "pattern": {"kind": "stripes", "color_a": [0,0,0]}}),
            "non-solid without color_b",
        ),
        (
            FormatId::Png,
            json!({"width": 8, "height": 8, "bit_depth": 12, "pattern": {"kind": "solid", "color_a": [0,0,0]}}),
            "bad bit depth",
        ),
        (
            FormatId::Png,
            json!({"width": 8, "height": 8, "animated": {"num_frames": 1}, "pattern": {"kind": "solid", "color_a": [0,0,0]}}),
            "single-frame animation",
        ),
        (
            FormatId::Jpeg,
            json!({"width": 70000, "height": 8, "pattern": {"kind": "solid", "color_a": [0,0,0]}}),
            "family dimension cap",
        ),
    ] {
        let error = image::ImageSpec::parse(&spec(format, params.clone()), format)
            .err()
            .unwrap_or_else(|| panic!("{label}: expected InvalidSpec for {params}"));
        assert!(
            matches!(error, MediaError::InvalidSpec),
            "{label}: got {error:?}"
        );
    }

    // The jpeg-specific 65535 dimension ceiling (format constraint,
    // exercised past the family-wide parse cap via a direct value).
    let wide = image::ImageSpec {
        width: 70_000,
        height: 8,
        color: image::ColorKind::Rgb,
        bit_depth: 8,
        pattern: image::Pattern {
            kind: image::PatternKind::Solid,
            color_a: vec![0, 0, 0],
            color_b: Vec::new(),
            cell: 8,
        },
        animated: None,
        exif: None,
        xmp: None,
        quality: 90.0,
        progressive: false,
        speed: 9,
        psnr_floor_db: None,
    };
    assert!(
        matches!(wide.check(FormatId::Jpeg), Err(MediaError::InvalidSpec)),
        "jpeg dim ceiling"
    );
    assert_invalid_spec(
        FormatId::Jpeg,
        json!({"width": 8, "height": 8, "color": "rgba", "pattern": {"kind": "solid", "color_a": [0,0,0,0]}}),
        "jpeg has no alpha",
    );
    assert_invalid_spec(
        FormatId::Jpeg,
        json!({"width": 8, "height": 8, "bit_depth": 16, "pattern": {"kind": "solid", "color_a": [0,0,0]}}),
        "jpeg has no 16-bit",
    );
    assert_invalid_spec(
        FormatId::Webp,
        json!({"width": 8, "height": 8, "bit_depth": 16, "pattern": {"kind": "solid", "color_a": [0,0,0]}}),
        "webp has no 16-bit",
    );
    assert_invalid_spec(
        FormatId::Avif,
        json!({"width": 8, "height": 8, "bit_depth": 16, "pattern": {"kind": "solid", "color_a": [0,0,0]}}),
        "avif lane is 8-bit in v0.2",
    );
    let error = image::ImageSpec::parse(
        &spec(FormatId::Avif, json!({"width": 8, "height": 8, "speed": 11, "pattern": {"kind": "solid", "color_a": [0,0,0]}})),
        FormatId::Avif,
    )
    .err()
    .unwrap_or_else(|| panic!("ravif speed out of range: expected InvalidSpec"));
    assert!(
        matches!(error, MediaError::InvalidSpec),
        "ravif speed out of range: got {error:?}"
    );
}

#[test]
fn png_exif_timestamps_pin_the_determinism_epoch() {
    // The determinism contract: EXIF DateTime fields default to the
    // fixed epoch, never the ambient clock.
    let request = spec(
        FormatId::Png,
        json!({
            "width": 8, "height": 8,
            "pattern": {"kind": "solid", "color_a": [0, 0, 0]},
            "exif": {"orientation": 1},
        }),
    );
    let bytes = image::png::generate(&request).expect("generates");
    let extracted = image::png::extract(&bytes, Default::default()).expect("extracts");
    // EXIF renders the epoch as YYYY:MM:DD HH:MM:SS.
    assert_eq!(
        extracted.tree["exif"]["fields"]["date_time"],
        json!("2026:01:01 00:00:00")
    );
}

#[test]
fn gradient_specs_of_every_color_kind_generate_valid_png() {
    for (color, arity) in [("rgb", 3), ("rgba", 4), ("gray", 1), ("gray_alpha", 2)] {
        let a: Vec<u8> = vec![10; arity];
        let b: Vec<u8> = vec![240; arity];
        let request = spec(
            FormatId::Png,
            json!({"width": 9, "height": 5, "color": color,
                   "pattern": {"kind": "gradient", "color_a": a, "color_b": b}}),
        );
        let bytes = image::png::generate(&request).unwrap_or_else(|e| panic!("{color}: {e:?}"));
        let tree = image::png::extract(&bytes, Default::default())
            .unwrap_or_else(|e| panic!("{color} extract: {e:?}"))
            .tree;
        assert_eq!(
            tree["color_type"],
            json!(color),
            "{color} color type round-trips"
        );
    }
}
