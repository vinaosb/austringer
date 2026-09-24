//! M11 svg generate tests: deterministic well-formed output (namespace
//! declared, attrs escaped, stable attribute order), typed InvalidSpec
//! refusals, and SVG-1.1-targeted emission. Hermetic.

use harness_media::adapter::svg;
use harness_media::error::MediaError;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::json;

fn spec(params: serde_json::Value) -> GenerationSpec {
    GenerationSpec { format: FormatId::Svg, params }
}

#[test]
fn generation_is_byte_deterministic_across_runs() {
    // Given: one non-trivial spec (all common attrs live).
    let input = json!({
        "viewBox": [0.0, 0.0, 100.0, 50.0],
        "title": "t",
        "desc": "d",
        "shapes": [
            {"type": "rect", "x": 1.0, "y": 2.0, "width": 30.0, "height": 40.0,
             "id": "r", "class": "c", "fill": "red", "stroke": "black",
             "stroke_width": 2.0, "opacity": 0.5, "transform": "translate(1,2)"},
            {"type": "text", "x": 5.0, "y": 8.0, "content": "hello",
             "font_size": 12.0, "font_family": "sans-serif"}
        ]
    });
    // When: generated twice.
    let first = svg::generate(&spec(input.clone())).expect("generate");
    let second = svg::generate(&spec(input)).expect("generate");
    // Then: byte-identical (the M25 byte-rep discipline starts here; svg
    // has no clock fields, so the determinism contract is unconditional).
    assert_eq!(first, second);
}

#[test]
fn output_declares_namespace_and_xml_declaration() {
    // Given: a minimal spec.
    let bytes = svg::generate(&spec(json!({"shapes": []}))).expect("generate");
    let text = std::str::from_utf8(&bytes).expect("svg is utf-8");
    // Then: XML declaration present...
    assert!(text.starts_with("<?xml "), "xml decl first: {text}");
    // ...root is svg with the SVG 1.1 namespace declared (plan: namespace
    // declared - a namespace-less svg is not a valid standalone document).
    assert!(
        text.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""),
        "namespace must be declared: {text}"
    );
}

#[test]
fn pinned_minimal_output_is_exactly_stable() {
    // Given: the smallest meaningful spec.
    let bytes = svg::generate(&spec(json!({
        "viewBox": [0.0, 0.0, 10.0, 10.0],
        "shapes": [{"type": "rect", "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0}]
    })))
    .expect("generate");
    // Then: the exact bytes are pinned - attribute order is stable
    // (namespace, viewBox, then geometry), no ambient whitespace.
    assert_eq!(
        std::str::from_utf8(&bytes).expect("utf-8"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\">\
         <rect x=\"1\" y=\"2\" width=\"3\" height=\"4\"/></svg>"
    );
}

#[test]
fn text_and_attributes_are_escaped() {
    // Given: content and attribute values carrying XML metacharacters.
    let bytes = svg::generate(&spec(json!({
        "title": "a<b & c>d",
        "shapes": [
            {"type": "rect", "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0,
             "id": "x\"y", "class": "a&b"},
            {"type": "text", "x": 1.0, "y": 2.0, "content": "1 < 2 & 3 > 2"}
        ]
    })))
    .expect("generate");
    let text = std::str::from_utf8(&bytes).expect("utf-8");
    // Then: attribute values escape quotes/ampersands...
    assert!(text.contains("id=\"x&quot;y\""), "attr escaping: {text}");
    assert!(text.contains("class=\"a&amp;b\""), "attr escaping: {text}");
    // ...text content escapes angle brackets and ampersands...
    assert!(text.contains("1 &lt; 2 &amp; 3 &gt; 2"), "text escaping: {text}");
    // ...title content is escaped too...
    assert!(text.contains("<title>a&lt;b &amp; c&gt;d</title>"), "title escaping: {text}");
    // ...and the document round-trips: extraction unescapes back.
    let extracted = svg::extract(&bytes, Default::default()).expect("extract");
    assert_eq!(extracted.tree["title"], json!("a<b & c>d"));
    assert_eq!(extracted.tree["text"], json!("1 < 2 & 3 > 2"));
    assert_eq!(extracted.tree["ids"], json!(["x\"y"]));
    assert_eq!(extracted.tree["classes"], json!(["a&b"]));
}

#[test]
fn every_shape_kind_emits_its_svg_1_1_element() {
    // Given: one of each of the eight supported shape kinds.
    let input = json!({
        "viewBox": [0.0, 0.0, 20.0, 20.0],
        "shapes": [
            {"type": "rect", "x": 1.0, "y": 1.0, "width": 2.0, "height": 2.0},
            {"type": "circle", "cx": 5.0, "cy": 5.0, "r": 1.0},
            {"type": "ellipse", "cx": 8.0, "cy": 8.0, "rx": 2.0, "ry": 1.0},
            {"type": "line", "x1": 10.0, "y1": 10.0, "x2": 12.0, "y2": 12.0},
            {"type": "polyline", "points": [1.0, 2.0, 3.0, 4.0]},
            {"type": "polygon", "points": [5.0, 6.0, 7.0, 8.0, 9.0, 6.0]},
            {"type": "path", "d": "M 1 1 L 2 2 Z"},
            {"type": "text", "x": 3.0, "y": 3.0, "content": "hi"}
        ]
    });
    // When: generated.
    let bytes = svg::generate(&spec(input)).expect("generate");
    let text = std::str::from_utf8(&bytes).expect("utf-8");
    // Then: each SVG 1.1 basic-shape element appears with its geometry
    // (path carries the d attribute verbatim; text carries content).
    assert!(text.contains("<rect x=\"1\" y=\"1\" width=\"2\" height=\"2\"/>"), "{text}");
    assert!(text.contains("<circle cx=\"5\" cy=\"5\" r=\"1\"/>"), "{text}");
    assert!(text.contains("<ellipse cx=\"8\" cy=\"8\" rx=\"2\" ry=\"1\"/>"), "{text}");
    assert!(text.contains("<line x1=\"10\" y1=\"10\" x2=\"12\" y2=\"12\"/>"), "{text}");
    assert!(text.contains("<polyline points=\"1 2 3 4\"/>"), "{text}");
    assert!(text.contains("<polygon points=\"5 6 7 8 9 6\"/>"), "{text}");
    assert!(text.contains("<path d=\"M 1 1 L 2 2 Z\"/>"), "{text}");
    assert!(text.contains("<text x=\"3\" y=\"3\" font-family=\"sans-serif\">hi</text>"), "{text}");
}

#[test]
fn optional_attrs_are_omitted_not_emptied() {
    // Given: a shape with no optional attrs set.
    let bytes = svg::generate(&spec(json!({
        "shapes": [{"type": "rect", "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}]
    })))
    .expect("generate");
    let text = std::str::from_utf8(&bytes).expect("utf-8");
    // Then: the ROOT tag carries no optional attrs (closes right after
    // the namespace: no viewBox, no width, no height) and the shape
    // carries no empty id/class/fill.
    assert!(
        text.contains("<svg xmlns=\"http://www.w3.org/2000/svg\">"),
        "root omits absent attrs entirely: {text}"
    );
    assert!(!text.contains("id=\"\""), "{text}");
    assert!(!text.contains("class=\"\""), "{text}");
    assert!(!text.contains("fill=\"\""), "{text}");
}

#[test]
fn title_and_desc_land_before_shapes() {
    // Given: a spec with title and desc.
    let bytes = svg::generate(&spec(json!({
        "title": "T", "desc": "D",
        "shapes": [{"type": "circle", "cx": 1.0, "cy": 1.0, "r": 1.0}]
    })))
    .expect("generate");
    let text = std::str::from_utf8(&bytes).expect("utf-8");
    // Then: title, then desc, then shapes (stable document order).
    let title = text.find("<title>").expect("title present");
    let desc = text.find("<desc>").expect("desc present");
    let circle = text.find("<circle").expect("circle present");
    assert!(title < desc && desc < circle, "document order: {text}");
}

#[test]
fn default_text_font_stack_is_common() {
    // Given: a text shape without an explicit family.
    let bytes = svg::generate(&spec(json!({
        "shapes": [{"type": "text", "x": 0.0, "y": 0.0, "content": "x"}]
    })))
    .expect("generate");
    let text = std::str::from_utf8(&bytes).expect("utf-8");
    // Then: the emitted family is a common stack (the digest's #1 render
    // divergence guard - no exotic font claims).
    assert!(
        text.contains("font-family=\"sans-serif\""),
        "common font stack: {text}"
    );
}

#[test]
fn invalid_specs_fail_typed_invalid_spec() {
    // Given: specs violating the contract in distinct ways.
    for params in [
        json!({"shapes": [{"type": "hexagon", "x": 0.0}]}),          // unknown kind
        json!({"shapes": [{"type": "rect", "x": 0.0, "y": 0.0, "width": -1.0, "height": 1.0}]}), // negative
        json!({"shapes": [{"type": "circle", "cx": 0.0, "cy": 0.0, "r": -2.0}]}),                // negative
        json!({"shapes": [{"type": "polyline", "points": [1.0, 2.0, 3.0]}]}),                    // odd points
        json!({"shapes": [{"type": "polygon", "points": [1.0, 2.0]}]}),                          // < 3 points
        json!({"shapes": [{"type": "path", "d": ""}]}),                                          // empty d
        json!({"viewBox": [0.0, 0.0, 10.0]}),                                                     // 3 numbers
        json!({"shapes": [{"type": "text", "x": 0.0, "y": 0.0, "content": "x", "opacity": 2.0}]}), // opacity range
        json!({"shapes": [{"type": "rect", "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0, "id": "dup"},
                          {"type": "rect", "x": 2.0, "y": 2.0, "width": 1.0, "height": 1.0, "id": "dup"}]}), // dup id
        json!({"width": -5.0, "shapes": []}),                                                     // negative width
    ] {
        // When: generated.
        let error = svg::generate(&spec(params))
            .expect_err("contract violation must refuse typed");
        // Then: InvalidSpec - never a half-written document.
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
}

#[test]
fn non_finite_numbers_fail_typed_invalid_spec() {
    // Given: NaN smuggled through JSON (serde_json maps it to null, which
    // must fail required-field validation, not coerce).
    let params: serde_json::Value =
        serde_json::from_str(r#"{"shapes": [{"type": "circle", "cx": null, "cy": 0.0, "r": 1.0}]}"#)
            .expect("json parses");
    let error = svg::generate(&spec(params)).expect_err("null geometry must refuse");
    assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
}

#[test]
fn wrong_format_spec_fails_typed_invalid_spec() {
    // Given: a well-formed svg spec addressed to another format.
    let mismatch = GenerationSpec { format: FormatId::Png, params: json!({"shapes": []}) };
    // When: generated through the svg adapter.
    let error = svg::generate(&mismatch).expect_err("format mismatch must refuse");
    // Then: typed InvalidSpec (the adapter owns one format).
    assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
}
