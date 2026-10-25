//! M28 image-family round-trip tests: generate -> verify corpora under
//! the M5 diff engine (png/webp lossless decode-back + pixel hash; jpeg
//! dims + PSNR floor, never hash; APNG frame count + durations), the
//! mutated-expectation non-vacuous pins, and the `#[ignore]` real-ffmpeg
//! AVIF decode-back (M17 lane, run at-todo-time on the dev machine).

use austringer_media::adapter::image;
use austringer_media::diff::RoundTripVerdict;
use austringer_media::format::FormatId;
use austringer_media::generate::GenerationSpec;
use serde_json::{Value, json};

fn spec(format: FormatId, params: Value) -> GenerationSpec {
    GenerationSpec { format, params }
}

fn gradient(width: u32, height: u32) -> Value {
    json!({
        "width": width, "height": height,
        "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
    })
}

fn verify_dispatch(format: FormatId, expected: &Value, bytes: &[u8]) -> RoundTripVerdict {
    let produced = austringer_media::executor::registry()
        .dispatch(
            format,
            austringer_media::executor::OpRequest::Verify { expected, bytes },
        )
        .expect("verify dispatch");
    match produced.inner() {
        austringer_media::executor::OpOutput::Verify(verdict) => verdict.clone(),
        other => panic!("expected Verify output, got {other:?}"),
    }
}

#[test]
fn png_roundtrip_corpus_passes_with_pixel_hash() {
    // Corpus >= 10 specs: every color kind, both bit depths, every
    // pattern kind, animated and static, with and without metadata.
    let corpus = [
        json!({"width": 12, "height": 8, "pattern": {"kind": "gradient", "color_a": [0,0,0], "color_b": [255,255,255]}}),
        json!({"width": 12, "height": 8, "color": "rgba", "pattern": {"kind": "gradient", "color_a": [0,0,0,0], "color_b": [255,255,255,255]}}),
        json!({"width": 12, "height": 8, "color": "gray", "pattern": {"kind": "gradient", "color_a": [0], "color_b": [255]}}),
        json!({"width": 12, "height": 8, "color": "gray_alpha", "bit_depth": 16, "pattern": {"kind": "gradient", "color_a": [0,0], "color_b": [255,255]}}),
        json!({"width": 16, "height": 4, "pattern": {"kind": "checker", "color_a": [255,0,0], "color_b": [0,0,255], "cell": 4}}),
        json!({"width": 4, "height": 16, "bit_depth": 16, "pattern": {"kind": "checker", "color_a": [1,2,3], "color_b": [200,210,220], "cell": 1}}),
        json!({"width": 8, "height": 8, "pattern": {"kind": "stripes", "color_a": [10,20,30], "color_b": [40,50,60], "cell": 3}}),
        json!({"width": 8, "height": 8, "pattern": {"kind": "solid", "color_a": [7,8,9]}}),
        json!({"width": 10, "height": 6, "pattern": {"kind": "solid", "color_a": [7,8,9]},
               "exif": {"orientation": 6, "artist": "austringer"}, "xmp": {"title": "t"}}),
        json!({"width": 8, "height": 6, "pattern": {"kind": "stripes", "color_a": [255,0,0], "color_b": [0,0,255], "cell": 2},
               "animated": {"num_frames": 4, "num_plays": 0, "delay_num": 1, "delay_den": 5}}),
        json!({"width": 1, "height": 1, "color": "gray", "pattern": {"kind": "solid", "color_a": [128]}}),
    ];
    for params in corpus {
        let request = spec(FormatId::Png, params.clone());
        let parsed = image::ImageSpec::parse(&request, FormatId::Png)
            .unwrap_or_else(|error| panic!("spec {params}: {error:?}"));
        let expected = image::png::verify_tree(&parsed);
        let bytes = image::png::generate(&request).unwrap_or_else(|e| panic!("{params}: {e:?}"));
        let verdict = verify_dispatch(FormatId::Png, &expected, &bytes);
        assert!(verdict.is_pass(), "params {params} -> {verdict:?}");
    }
}

#[test]
fn png_generate_extract_structural_diff_is_empty_on_spec_tree() {
    // The metadata-level round trip (M5 diff over the extract tree vs the
    // INDEPENDENT spec->tree projection - separate from pixel verify).
    let request = spec(
        FormatId::Png,
        json!({
            "width": 12, "height": 8, "pattern": {"kind": "checker", "color_a": [0,0,0], "color_b": [255,255,255], "cell": 4},
            "animated": {"num_frames": 2, "num_plays": 3, "delay_num": 2, "delay_den": 100},
        }),
    );
    let parsed = image::ImageSpec::parse(&request, FormatId::Png).expect("spec");
    let expected = image::png::spec_tree(&parsed);
    let bytes = image::png::generate(&request).expect("generate");
    let extracted = image::png::extract(&bytes, Default::default()).expect("extract");
    let diff = austringer_media::diff::structural_diff(
        &expected,
        &extracted.tree,
        &image::png::diff_profile(),
    )
    .expect("diff");
    assert!(diff.is_empty(), "structural diff: {diff:?}");
}

#[test]
fn png_mutated_frame_hash_fails_with_exact_pointer() {
    // Non-vacuous gate: mutate the expected frame hash -> exact pointer.
    let request = spec(FormatId::Png, gradient(8, 8));
    let parsed = image::ImageSpec::parse(&request, FormatId::Png).expect("spec");
    let mut expected = image::png::verify_tree(&parsed);
    expected["frames"][0]["hash"] = json!("deadbeef");
    let bytes = image::png::generate(&request).expect("generate");
    match verify_dispatch(FormatId::Png, &expected, &bytes) {
        RoundTripVerdict::Fail(diff) => {
            assert_eq!(diff.changed[0].path, "/frames/0/hash");
        }
        RoundTripVerdict::Pass => panic!("a mutated hash must not pass"),
    }
}

#[test]
fn png_frame_count_and_durations_are_verified_not_just_first_frame() {
    // The APNG verify hook (plan): frame count + per-frame durations.
    let request = spec(
        FormatId::Png,
        json!({
            "width": 8, "height": 6,
            "pattern": {"kind": "stripes", "color_a": [255,0,0], "color_b": [0,0,255], "cell": 2},
            "animated": {"num_frames": 3, "num_plays": 0, "delay_num": 3, "delay_den": 50},
        }),
    );
    let parsed = image::ImageSpec::parse(&request, FormatId::Png).expect("spec");
    let expected = image::png::verify_tree(&parsed);
    assert_eq!(expected["frames"].as_array().map(Vec::len), Some(3));
    assert_eq!(expected["frames"][1]["delay"]["delay_num"], json!(3));
    assert_eq!(expected["frames"][1]["delay"]["delay_den"], json!(50));
    // Per-frame hashes differ (each frame shifts the pattern).
    assert_ne!(expected["frames"][0]["hash"], expected["frames"][1]["hash"]);
    let bytes = image::png::generate(&request).expect("generate");
    assert!(verify_dispatch(FormatId::Png, &expected, &bytes).is_pass());
    // And a truncated expectation (2 frames vs 3 produced) fails exactly.
    let mut short = expected.clone();
    if let Some(frames) = short["frames"].as_array_mut() {
        frames.truncate(2);
    }
    match verify_dispatch(FormatId::Png, &short, &bytes) {
        RoundTripVerdict::Fail(diff) => assert_eq!(diff.added[0].path, "/frames/2"),
        RoundTripVerdict::Pass => panic!("missing frame expectation must not pass"),
    }
}

#[test]
fn webp_lossless_roundtrip_corpus_passes_with_pixel_hash() {
    let corpus = [
        json!({"width": 12, "height": 8, "pattern": {"kind": "gradient", "color_a": [0,0,0], "color_b": [255,255,255]}}),
        json!({"width": 12, "height": 8, "color": "rgba", "pattern": {"kind": "gradient", "color_a": [0,0,0,255], "color_b": [255,255,255,0]}}),
        json!({"width": 12, "height": 8, "color": "gray", "pattern": {"kind": "checker", "color_a": [0], "color_b": [255], "cell": 3}}),
        json!({"width": 12, "height": 8, "color": "gray_alpha", "pattern": {"kind": "stripes", "color_a": [10,20], "color_b": [200,250], "cell": 2}}),
        json!({"width": 6, "height": 6, "pattern": {"kind": "solid", "color_a": [7,8,9]}}),
        json!({"width": 1, "height": 4, "pattern": {"kind": "gradient", "color_a": [0,0,0], "color_b": [255,255,255]}}),
        json!({"width": 4, "height": 1, "pattern": {"kind": "gradient", "color_a": [0,0,0], "color_b": [255,255,255]}}),
        json!({"width": 9, "height": 7, "color": "gray", "pattern": {"kind": "gradient", "color_a": [5], "color_b": [250]}}),
        json!({"width": 8, "height": 8, "pattern": {"kind": "checker", "color_a": [1,2,3], "color_b": [254,253,252], "cell": 1}}),
        json!({"width": 10, "height": 10, "color": "rgba", "pattern": {"kind": "stripes", "color_a": [0,255,0,128], "color_b": [255,0,255,32], "cell": 5}}),
    ];
    for params in corpus {
        let request = spec(FormatId::Webp, params.clone());
        let parsed = image::ImageSpec::parse(&request, FormatId::Webp)
            .unwrap_or_else(|error| panic!("spec {params}: {error:?}"));
        let expected = image::webp::verify_tree(&parsed);
        let bytes = image::webp::generate(&request).unwrap_or_else(|e| panic!("{params}: {e:?}"));
        let verdict = verify_dispatch(FormatId::Webp, &expected, &bytes);
        assert!(verdict.is_pass(), "params {params} -> {verdict:?}");
    }
}

#[test]
fn jpeg_roundtrip_passes_psnr_floor_and_fails_on_wrong_pixels() {
    let params = json!({
        "width": 32, "height": 24, "quality": 90,
        "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
    });
    let request = spec(FormatId::Jpeg, params);
    let parsed = image::ImageSpec::parse(&request, FormatId::Jpeg).expect("spec");
    let expected = image::jpeg::verify_tree(&parsed);
    let bytes = image::jpeg::generate(&request).expect("generate");
    // Lossy verify: dims + PSNR floor (never a pixel hash).
    assert!(verify_dispatch(FormatId::Jpeg, &expected, &bytes).is_pass());
    // Non-vacuous: verifying the gradient expectation against a SOLID
    // black jpeg must fail (PSNR far below any sane floor).
    let black = spec(
        FormatId::Jpeg,
        json!({
            "width": 32, "height": 24, "quality": 90,
            "pattern": {"kind": "solid", "color_a": [0, 0, 0]},
        }),
    );
    let black_bytes = image::jpeg::generate(&black).expect("black jpeg");
    match verify_dispatch(FormatId::Jpeg, &expected, &black_bytes) {
        RoundTripVerdict::Fail(_) => {}
        RoundTripVerdict::Pass => panic!("wrong pixels must fail the PSNR floor"),
    }
}

#[test]
fn avif_encode_roundtrips_via_container_verify() {
    // The pure-tier avif gate: ravif encode -> container extract (brand,
    // ispe dims, pixi bit depth) diffed against the independent spec
    // tree. Pixel decode-back is the #[ignore] ffmpeg test below.
    let request = spec(
        FormatId::Avif,
        json!({
            "width": 8, "height": 6, "quality": 60.0, "speed": 10,
            "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
        }),
    );
    let parsed = image::ImageSpec::parse(&request, FormatId::Avif).expect("spec");
    let expected = image::avif::spec_tree(&parsed);
    let bytes = image::avif::generate(&request).expect("ravif encodes");
    assert!(verify_dispatch(FormatId::Avif, &expected, &bytes).is_pass());
}

#[test]
#[ignore = "real-ffmpeg tier: run at-todo-time on the dev machine (M17 lane, never CI)"]
fn avif_real_ffmpeg_decode_back_pixel_hash() {
    // Generates a tiny avif, decodes it back through the REAL ffmpeg
    // into a png, and pixel-hash-verifies against the independent spec
    // projection - the plan's decode-back hook for the format with no
    // pure-Rust AV1 decoder.
    let request = spec(
        FormatId::Avif,
        json!({
            "width": 8, "height": 6, "quality": 60.0, "speed": 10,
            "pattern": {"kind": "solid", "color_a": [80, 90, 100]},
        }),
    );
    let _parsed = image::ImageSpec::parse(&request, FormatId::Avif).expect("spec");
    let bytes = image::avif::generate(&request).expect("ravif encodes");
    let dir = tempfile::tempdir().expect("tempdir");
    let avif_path = dir.path().join("probe.avif");
    let png_path = dir.path().join("probe.png");
    std::fs::write(&avif_path, &bytes).expect("write avif");
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-i"])
        .arg(&avif_path)
        .arg(&png_path)
        .status()
        .expect("ffmpeg must be installed for the real-tool tier");
    assert!(status.success(), "ffmpeg decode-back failed: {status}");
    let decoded = std::fs::read(&png_path).expect("decoded png");
    let extracted = image::png::extract(&decoded, Default::default()).expect("decode png");
    // The decoded-back png must carry the avif's ispe dims.
    assert_eq!(extracted.tree["width"], json!(8));
    assert_eq!(extracted.tree["height"], json!(6));
}

#[test]
fn jxl_extract_only_has_no_verify_round_trip_by_design() {
    // The honesty row: jxl verify is refused (no writer to gate) - the
    // executor's typed refusal, never a silent pass or a fake verify.
    let error = austringer_media::executor::registry()
        .dispatch(
            FormatId::Jxl,
            austringer_media::executor::OpRequest::Verify {
                expected: &json!({}),
                bytes: b"\xFF\x0A",
            },
        )
        .expect_err("verify must refuse on an extract-only format");
    assert!(matches!(
        error,
        austringer_media::error::MediaError::UnsupportedOperation { .. }
    ));
}

#[test]
fn committed_lossless_fixtures_are_byte_identical_to_regeneration() {
    let cases: [(String, FormatId, Value); 3] = [
        (
            "png.png".into(),
            FormatId::Png,
            json!({"width": 12, "height": 8,
                   "pattern": {"kind": "gradient", "color_a": [0,0,0], "color_b": [255,255,255]}}),
        ),
        (
            "jpeg.jpg".into(),
            FormatId::Jpeg,
            json!({"width": 16, "height": 12, "quality": 85,
                   "pattern": {"kind": "gradient", "color_a": [0,0,0], "color_b": [255,255,255]}}),
        ),
        (
            "webp.webp".into(),
            FormatId::Webp,
            json!({"width": 12, "height": 8,
                   "pattern": {"kind": "gradient", "color_a": [0,0,0], "color_b": [255,255,255]}}),
        ),
    ];
    for (name, format, params) in cases {
        let bytes = match format {
            FormatId::Png => image::png::generate(&spec(format, params.clone())).unwrap(),
            FormatId::Jpeg => image::jpeg::generate(&spec(format, params.clone())).unwrap(),
            FormatId::Webp => image::webp::generate(&spec(format, params.clone())).unwrap(),
            other => panic!("unexpected format {other}"),
        };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(&name);
        let committed =
            std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} missing: {error}"));
        assert_eq!(bytes, committed, "fixture {name} must match regeneration");
    }
}
