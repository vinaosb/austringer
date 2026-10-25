//! M11 svg extract tests: bounded structure tree, text projection bound,
//! adversarial inputs (malformed, XXE, billion-laughs), and extract
//! budget enforcement. Hermetic: fixtures are committed bytes, nothing is
//! fetched, no filesystem reads beyond the fixture files.

use austringer_media::adapter::svg;
use austringer_media::error::MediaError;
use austringer_media::extract::ExtractBudgets;
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} missing: {error}"))
}

#[test]
fn minimal_fixture_extracts_pinned_tree() {
    // Given: the M2 minimal fixture (root attrs + one rect).
    let bytes = fixture("svg.svg");
    // When: extracted under default budgets.
    let extracted =
        svg::extract(&bytes, ExtractBudgets::default()).expect("minimal svg must extract");
    // Then: the canonical tree is the pinned bounded summary (this
    // fixture carries width/height but no viewBox - absent stays absent).
    assert_eq!(
        extracted.tree,
        json!({
            "animation_elements": 0,
            "census": {"rect": 1, "svg": 1},
            "classes": [],
            "embedded_rasters": [],
            "ids": [],
            "root": {"height": 4.0, "width": 4.0},
            "text": "",
        })
    );
    // And: no thumbnails (rasterization is out of scope in v0.2 - honest).
    assert!(extracted.thumbnails.is_empty());
}

#[test]
fn rich_fixture_extracts_census_ids_classes_text_rasters_animations() {
    // Given: the rich fixture (ids, classes, title/desc, text with an
    // entity reference, two embedded raster refs, three animation
    // elements).
    let bytes = fixture("svg-rich.svg");
    // When: extracted.
    let extracted = svg::extract(&bytes, ExtractBudgets::default()).expect("rich svg must extract");
    // Then: every bounded field is pinned.
    assert_eq!(
        extracted.tree,
        json!({
            "animation_elements": 3,
            "census": {
                "animate": 1, "animateTransform": 1, "circle": 1, "desc": 1,
                "g": 1, "image": 2, "rect": 3, "set": 1, "svg": 1, "text": 1,
                "title": 1,
            },
            "classes": ["caption", "chart", "dark", "data", "frame", "marker"],
            "embedded_rasters": [
                "data:image/png;base64,iVBORw0KGgo=",
                "tiny-logo.png",
            ],
            "ids": ["bars", "dot", "frame", "label", "q1", "q2", "root"],
            "root": {"viewBox": [0.0, 0.0, 100.0, 50.0]},
            "text": "Quarter & year",
            "title": "Quarterly chart",
            "desc": "Bars per quarter",
        })
    );
}

/// The per-format bound never exceeds the M1 hard ceiling (compile-time
/// law; a violation fails the build, not a runtime test run).
const _: () = {
    assert!(svg::TEXT_PROJECTION_MAX_CHARS <= austringer_media::extract::TEXT_PROJECTION_MAX_CHARS);
};

#[test]
fn text_projection_is_bounded_deterministic_and_informative() {
    // Given: the rich fixture.
    let bytes = fixture("svg-rich.svg");
    // When: extracted twice.
    let first = svg::extract(&bytes, ExtractBudgets::default()).expect("extract");
    let second = svg::extract(&bytes, ExtractBudgets::default()).expect("extract");
    // Then: the projection is deterministic...
    assert_eq!(first.text_projection, second.text_projection);
    // ...bounded by the svg per-format default (1200 chars, never tokens)...
    assert!(
        first.text_projection.chars().count() <= svg::TEXT_PROJECTION_MAX_CHARS,
        "projection must respect the per-format char bound"
    );
    // ...and informative enough for a text-only model to answer structure
    // questions (dimensions, census, text content).
    let projection = first.text_projection;
    assert!(
        projection.contains("100"),
        "viewBox width surfaces: {projection}"
    );
    assert!(
        projection.contains("rect=3"),
        "census surfaces: {projection}"
    );
    assert!(
        projection.contains("Quarter & year"),
        "text content surfaces: {projection}"
    );
}

#[test]
fn oversized_text_content_is_truncated_within_the_char_bound() {
    // Given: an svg whose text content far exceeds the projection bound.
    let huge = "x".repeat(5000);
    let bytes = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><text x=\"0\" y=\"10\">{huge}</text></svg>"
    );
    // When: extracted.
    let extracted = svg::extract(bytes.as_bytes(), ExtractBudgets::default()).expect("extract");
    // Then: projection and tree text stay within the bound (chars, at a
    // char boundary - never a panicked half-codepoint).
    assert!(extracted.text_projection.chars().count() <= svg::TEXT_PROJECTION_MAX_CHARS);
    let tree_text = extracted.tree["text"].as_str().expect("text field");
    assert!(tree_text.chars().count() <= svg::TEXT_PROJECTION_MAX_CHARS);
}

#[test]
fn malformed_xml_fails_typed_parse_failure() {
    // Given: truncated and mismatched documents.
    for bytes in [
        &b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect"[..],
        &b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect></svg>"[..],
        &b"<svg><rect x=\"1\""[..],
    ] {
        // When: extracted.
        let error = svg::extract(bytes, ExtractBudgets::default())
            .expect_err("malformed xml must fail typed");
        // Then: ParseFailure - never a panic, never a partial tree.
        assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
    }
}

#[test]
fn non_svg_root_and_non_utf8_fail_typed() {
    // Given: well-formed XML that is not an svg, and non-utf8 bytes.
    for bytes in [
        &b"<foo/>"[..],
        &b"<html><body/></html>"[..],
        &[0xFF, 0xFE, 0x00, 0x00],
    ] {
        let error = svg::extract(bytes, ExtractBudgets::default())
            .expect_err("non-svg / non-utf8 must fail typed");
        assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
    }
}

#[test]
fn xxe_external_entity_is_refused_typed() {
    // Given: the committed XXE fixture (DOCTYPE + external SYSTEM entity).
    let bytes = fixture("svg-xxe.svg");
    // When: extracted.
    let error = svg::extract(&bytes, ExtractBudgets::default())
        .expect_err("external entities must be refused");
    // Then: typed ParseFailure - the document is never resolved, no file
    // is read, no network is touched (test-pinned XXE guard).
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn billion_laughs_is_refused_typed_and_never_expanded() {
    // Given: the committed billion-laughs fixture (nested internal
    // entities sized to blow up quadratically if expanded).
    let bytes = fixture("svg-billion-laughs.svg");
    // When: extracted under a normal budget.
    let started = std::time::Instant::now();
    let error = svg::extract(&bytes, ExtractBudgets::default())
        .expect_err("entity-declaring documents must be refused");
    let elapsed = started.elapsed();
    // Then: typed refusal, fast - entities are never expanded so the
    // quadratic blowup cannot even begin.
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "refusal must be immediate, took {elapsed:?}"
    );
}

#[test]
fn size_budget_trip_is_typed_extract_too_large() {
    // Given: a document collecting far more ids than a tiny size cap
    // allows the extractor to materialize.
    let mut bytes = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\"");
    for index in 0..500 {
        bytes.push_str(&format!(" id{index}=\"x\""));
    }
    bytes.push_str("/>");
    let budgets = ExtractBudgets {
        deadline: austringer_media::extract::DEFAULT_EXTRACT_DEADLINE,
        max_total_bytes: 64,
    };
    // When: extracted under the tiny cap.
    let error = svg::extract(bytes.as_bytes(), budgets).expect_err("cap must trip typed");
    // Then: ExtractTooLarge - the typed budget error, never a hang.
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn deadline_trip_is_typed_extract_too_large() {
    // Given: a valid minimal document and an already-expired deadline.
    let bytes = fixture("svg.svg");
    let budgets = ExtractBudgets {
        deadline: std::time::Duration::ZERO,
        max_total_bytes: austringer_media::extract::DEFAULT_EXTRACT_MAX_TOTAL_BYTES,
    };
    // When: extracted.
    let error = svg::extract(&bytes, budgets).expect_err("deadline must trip typed");
    // Then: ExtractTooLarge (the M1 contract covers both budget axes).
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn percent_width_height_stay_strings_and_unordered_input_normalizes() {
    // Given: attrs that are not plain numbers ("100%") and unsorted
    // id/class ordering.
    let bytes = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100%\" height=\"100%\" \
                  viewBox=\"0 0 10 10\"><rect id=\"z\" class=\"b a\"/><rect id=\"a\"/></svg>";
    // When: extracted.
    let extracted = svg::extract(bytes, ExtractBudgets::default()).expect("extract");
    // Then: non-numeric root attrs stay raw strings, id/class lists are
    // canonical (sorted, unique).
    assert_eq!(extracted.tree["root"]["width"], json!("100%"));
    assert_eq!(extracted.tree["root"]["height"], json!("100%"));
    assert_eq!(extracted.tree["ids"], json!(["a", "z"]));
    assert_eq!(extracted.tree["classes"], json!(["a", "b"]));
}
