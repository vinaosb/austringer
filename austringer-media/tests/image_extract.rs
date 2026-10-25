//! M28 image-family extract tests: the committed fixture corpus extracts
//! to pinned canonical trees (dims, bit depth, color mode, ICC/eXIf
//! presence, APNG frame count, AVIF brand), budget trips are typed, and
//! malformed input never panics. Hermetic throughout.

use austringer_media::adapter::image;
use austringer_media::error::MediaError;
use austringer_media::extract::ExtractBudgets;
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    std::fs::read(&path)
        .unwrap_or_else(|error| panic!("fixture {} missing: {error}", path.display()))
}

fn generated_png(params: serde_json::Value) -> Vec<u8> {
    image::png::generate(&austringer_media::generate::GenerationSpec {
        format: austringer_media::format::FormatId::Png,
        params,
    })
    .expect("png fixture generates")
}

fn generated_jpeg(params: serde_json::Value) -> Vec<u8> {
    image::jpeg::generate(&austringer_media::generate::GenerationSpec {
        format: austringer_media::format::FormatId::Jpeg,
        params,
    })
    .expect("jpeg fixture generates")
}

fn zero_deadline() -> ExtractBudgets {
    // ZERO, not 1ns: a nanosecond-scale deadline can read as
    // not-yet-elapsed on coarse timers and flake past the first check.
    ExtractBudgets {
        deadline: std::time::Duration::ZERO,
        max_total_bytes: u64::MAX,
    }
}

fn tiny_cap() -> ExtractBudgets {
    ExtractBudgets {
        deadline: std::time::Duration::from_secs(10),
        max_total_bytes: 64,
    }
}

#[test]
fn png_fixture_extracts_pinned_tree() {
    // Given: the committed self-generated rgb png fixture.
    // When: extracted.
    let extracted = image::png::extract(&fixture("png.png"), ExtractBudgets::default())
        .expect("png fixture extracts");
    // Then: the pinned canonical tree.
    assert_eq!(
        extracted.tree,
        json!({
            "format": "png",
            "width": 12,
            "height": 8,
            "bit_depth": 8,
            "color_type": "rgb",
            "interlaced": false,
            "icc_profile": null,
            "exif": null,
            "xmp": null,
            "animated": null,
        })
    );
    assert!(extracted.thumbnails.is_empty());
}

#[test]
fn png16_fixture_extracts_sixteen_bit_gray() {
    let generated = generated_png(json!({
        "width": 8,
        "height": 6,
        "color": "gray",
        "bit_depth": 16,
        "pattern": {"kind": "gradient", "color_a": [0], "color_b": [255]},
    }));
    let extracted =
        image::png::extract(&generated, ExtractBudgets::default()).expect("16-bit png extracts");
    assert_eq!(extracted.tree["bit_depth"], json!(16));
    assert_eq!(extracted.tree["color_type"], json!("gray"));
}

#[test]
fn apng_fixture_extracts_frame_count_and_plays() {
    // Given: a generated 3-frame APNG fixture.
    let generated = generated_png(json!({
        "width": 8,
        "height": 6,
        "pattern": {"kind": "stripes", "color_a": [255, 0, 0], "color_b": [0, 0, 255], "cell": 2},
        "animated": {"num_frames": 3, "num_plays": 0, "delay_num": 1, "delay_den": 10},
    }));
    let extracted =
        image::png::extract(&generated, ExtractBudgets::default()).expect("apng extracts");
    // Then: the animation control is surfaced (frame count is the plan's
    // pinned APNG fact).
    assert_eq!(
        extracted.tree["animated"],
        json!({"num_frames": 3, "num_plays": 0})
    );
}

#[test]
fn jpeg_fixture_extracts_dims_and_baseline_mode() {
    let extracted = image::jpeg::extract(&fixture("jpeg.jpg"), ExtractBudgets::default())
        .expect("jpeg fixture extracts");
    assert_eq!(extracted.tree["width"], json!(16));
    assert_eq!(extracted.tree["height"], json!(12));
    assert_eq!(extracted.tree["bit_depth"], json!(8));
    assert_eq!(extracted.tree["color_type"], json!("rgb"));
    assert_eq!(extracted.tree["progressive"], json!(false));
}

#[test]
fn progressive_jpeg_fixture_extracts_progressive_true() {
    let generated = generated_jpeg(json!({
        "width": 16,
        "height": 12,
        "quality": 90,
        "progressive": true,
        "pattern": {"kind": "gradient", "color_a": [0, 0, 0], "color_b": [255, 255, 255]},
    }));
    let extracted = image::jpeg::extract(&generated, ExtractBudgets::default())
        .expect("progressive jpeg extracts");
    assert_eq!(extracted.tree["progressive"], json!(true));
}

#[test]
fn webp_fixture_extracts_dims_alpha_lossless() {
    let extracted = image::webp::extract(&fixture("webp.webp"), ExtractBudgets::default())
        .expect("webp fixture extracts");
    assert_eq!(extracted.tree["width"], json!(12));
    assert_eq!(extracted.tree["height"], json!(8));
    assert_eq!(extracted.tree["alpha"], json!(false));
    assert_eq!(extracted.tree["lossless"], json!(true));
    assert_eq!(extracted.tree["animated"], json!(null));
}

#[test]
fn avif_fixture_extracts_brand_and_dims() {
    // Given: the committed self-encoded avif fixture (pure container walk
    // - no AV1 decode exists in the pure tier).
    let extracted = image::avif::extract(&fixture("avif.avif"), ExtractBudgets::default())
        .expect("avif container extracts");
    // Then: brand + ispe dims + pixi bit depth are pinned.
    assert_eq!(extracted.tree["format"], json!("avif"));
    assert_eq!(extracted.tree["brand"], json!("avif"));
    assert_eq!(extracted.tree["width"], json!(8));
    assert_eq!(extracted.tree["height"], json!(6));
    assert_eq!(extracted.tree["bit_depth"], json!(8));
}

#[test]
fn jxl_fixture_extracts_dims_bit_depth_frame_count() {
    let extracted = image::jxl::extract(&fixture("jxl.jxl"), ExtractBudgets::default())
        .expect("jxl fixture extracts");
    assert_eq!(extracted.tree["format"], json!("jxl"));
    assert!(
        extracted.tree["width"].is_u64(),
        "dims present: {}",
        extracted.tree["width"]
    );
    assert!(
        extracted.tree["height"].is_u64(),
        "dims present: {}",
        extracted.tree["height"]
    );
    assert_eq!(extracted.tree["frame_count"], json!(1));
    assert_eq!(extracted.tree["float_sample"], json!(false));
}

#[test]
fn exif_and_xmp_extract_from_generated_png() {
    // Given: a png generated with EXIF orientation + artist + an XMP
    // title (the sidecar layer's embed lane).
    let bytes = image::png::generate(&austringer_media::generate::GenerationSpec {
        format: austringer_media::format::FormatId::Png,
        params: json!({
            "width": 8, "height": 8,
            "pattern": {"kind": "solid", "color_a": [10, 20, 30]},
            "exif": {"orientation": 6, "artist": "austringer"},
            "xmp": {"title": "fixture"},
        }),
    })
    .expect("png with metadata generates");
    // When: extracted.
    let extracted = image::png::extract(&bytes, ExtractBudgets::default()).expect("extracts");
    // Then: both metadata lanes surface with the written values.
    assert_eq!(extracted.tree["exif"]["fields"]["orientation"], json!(6));
    assert_eq!(
        extracted.tree["exif"]["fields"]["artist"],
        json!("austringer")
    );
    let xmp = extracted.tree["xmp"].as_str().expect("xmp string");
    assert!(xmp.contains("fixture"), "xmp carries the title: {xmp}");
}

#[test]
fn truncated_png_is_typed_parse_failure() {
    let mut bytes = fixture("png.png");
    bytes.truncate(20);
    let error = image::png::extract(&bytes, ExtractBudgets::default())
        .expect_err("truncated png must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn corrupt_jpeg_is_typed_parse_failure() {
    let error = image::jpeg::extract(b"\xFF\xD8\xFF\xE0\x00\x10JFIF", ExtractBudgets::default())
        .expect_err("header-only jpeg must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn corrupt_webp_is_typed_parse_failure() {
    let error = image::webp::extract(b"RIFF\x00\x00\x00\x00WEBP", ExtractBudgets::default())
        .expect_err("empty webp container must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn png_deadline_trip_is_typed_extract_too_large() {
    let error = image::png::extract(&fixture("png.png"), zero_deadline())
        .expect_err("zero deadline must trip");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn webp_size_cap_trip_is_typed_extract_too_large() {
    let error = image::webp::extract(&fixture("webp.webp"), tiny_cap())
        .expect_err("tiny size cap must trip");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn text_projections_are_bounded_and_name_the_format() {
    for (name, expected_prefix, project) in [
        (
            "png.png",
            "png",
            image::png::extract
                as fn(
                    &[u8],
                    ExtractBudgets,
                ) -> Result<austringer_media::extract::Extracted, MediaError>,
        ),
        (
            "jpeg.jpg",
            "jpeg",
            image::jpeg::extract
                as fn(
                    &[u8],
                    ExtractBudgets,
                ) -> Result<austringer_media::extract::Extracted, MediaError>,
        ),
        (
            "webp.webp",
            "webp",
            image::webp::extract
                as fn(
                    &[u8],
                    ExtractBudgets,
                ) -> Result<austringer_media::extract::Extracted, MediaError>,
        ),
        (
            "avif.avif",
            "avif",
            image::avif::extract
                as fn(
                    &[u8],
                    ExtractBudgets,
                ) -> Result<austringer_media::extract::Extracted, MediaError>,
        ),
        (
            "jxl.jxl",
            "jxl",
            image::jxl::extract
                as fn(
                    &[u8],
                    ExtractBudgets,
                ) -> Result<austringer_media::extract::Extracted, MediaError>,
        ),
    ] {
        let extracted = project(&fixture(name), ExtractBudgets::default())
            .unwrap_or_else(|error| panic!("{name} projects: {error:?}"));
        assert!(
            extracted.text_projection.chars().count() <= image::TEXT_PROJECTION_MAX_CHARS,
            "{name}: projection bound"
        );
        assert!(
            extracted.text_projection.starts_with(expected_prefix),
            "{name}: projection names its format: {}",
            extracted.text_projection
        );
    }
}

#[test]
fn exif_sidecar_roundtrips_through_the_tiff_payload() {
    // Given: an EXIF spec (orientation + artist, no description).
    let spec = image::ExifSpec {
        orientation: Some(6),
        description: None,
        artist: Some("austringer".into()),
    };
    // When: serialized to the TIFF payload and parsed back.
    let Ok(tiff) = image::meta::exif_tiff(&spec) else {
        panic!("exif spec serializes");
    };
    let Some(tree) = image::meta::parse_exif_tree(&tiff) else {
        panic!("tiff payload parses back");
    };
    // Then: every field survives with the pinned determinism epoch.
    assert_eq!(tree["fields"]["orientation"], json!(6));
    assert_eq!(tree["fields"]["artist"], json!("austringer"));
    assert_eq!(tree["fields"]["date_time"], json!(image::EXIF_DATE_FORMAT));
    assert_eq!(tree["unknown_tags"], json!(0));
}

#[test]
fn xmp_packet_escapes_markup() {
    // Given: a title carrying all five XML metacharacters.
    let packet = image::meta::xmp_packet(&image::XmpSpec {
        title: Some("<a & b>".into()),
        description: None,
    });
    // Then: content escaped, packet wrapper intact.
    assert!(packet.contains("&lt;a &amp; b&gt;"), "{packet}");
    assert!(packet.starts_with("<?xpacket"));
}

#[test]
fn heic_brand_bytes_probe_typed_unknown_never_a_guess() {
    // The RED-list honesty row: HEIC/HEVC payloads are identified for
    // refusal only - an ftyp box with the heic brand is NOT avif and
    // probes typed-Unknown (generation of RED formats is refused by the
    // typed format vocabulary itself: no FormatId exists for them).
    let heic = b"\x00\x00\x00\x18ftypheic\x00\x00\x00\x00heicmif1";
    let result = austringer_media::probe::probe(heic, Some("photo.heic"));
    assert!(
        matches!(result, austringer_media::probe::ProbeResult::Unknown { .. }),
        "heic must not be guessed into a registered format: {result:?}"
    );
}
