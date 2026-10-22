//! M11 svg round-trip acceptance: generate -> extract -> structural diff
//! empty on a spec corpus of >= 10, with expected trees from the
//! INDEPENDENT spec->tree projection (never the adapter's own extractor
//! output), plus the usvg strict-parse stage-1 verify hook. Hermetic.

use harness_media::adapter::svg::{self, SvgSpec};
use harness_media::diff::RoundTripVerdict;
use harness_media::executor::{OpOutput, OpRequest};
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::{Value, json};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Svg,
        params,
    }
}

/// The spec corpus: >= 10 specs covering viewBox/title/desc presence,
/// every shape kind, multi-shape documents, unicode text, and the empty
/// document. Each row is (name, params).
fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        ("empty", json!({"shapes": []})),
        (
            "viewbox-only",
            json!({"viewBox": [0.0, 0.0, 100.0, 50.0], "shapes": []}),
        ),
        (
            "width-height",
            json!({"width": 640.0, "height": 480.0, "shapes": []}),
        ),
        (
            "title-desc",
            json!({"title": "T", "desc": "D", "shapes": []}),
        ),
        (
            "single-rect",
            json!({"viewBox": [0.0, 0.0, 10.0, 10.0],
                   "shapes": [{"type": "rect", "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0}]}),
        ),
        (
            "circle-attrs",
            json!({"viewBox": [0.0, 0.0, 10.0, 10.0],
                   "shapes": [{"type": "circle", "cx": 5.0, "cy": 5.0, "r": 2.0,
                               "id": "dot", "class": "marker", "fill": "red",
                               "stroke": "black", "stroke_width": 0.5, "opacity": 0.9}]}),
        ),
        (
            "ellipse-line",
            json!({"shapes": [
                {"type": "ellipse", "cx": 4.0, "cy": 2.0, "rx": 3.0, "ry": 1.5},
                {"type": "line", "x1": 0.0, "y1": 0.0, "x2": 4.0, "y2": 4.0}]}),
        ),
        (
            "polylines-polygon",
            json!({"shapes": [
                {"type": "polyline", "points": [0.0, 0.0, 1.5, 2.5, 3.0, 0.5]},
                {"type": "polygon", "points": [5.0, 5.0, 6.0, 7.0, 7.0, 5.0]}]}),
        ),
        (
            "path-and-text",
            json!({"viewBox": [0.0, 0.0, 20.0, 20.0], "title": "P", "desc": "path + text",
                   "shapes": [
                       {"type": "path", "d": "M 1 1 L 9 9 Z", "id": "p"},
                       {"type": "text", "x": 2.0, "y": 18.0, "content": "hello svg",
                        "font_size": 4.0}]}),
        ),
        (
            "unicode-text",
            json!({"shapes": [{"type": "text", "x": 1.0, "y": 1.0,
                               "content": "héllo wörld — 日本語 ✓"}]}),
        ),
        (
            "escaped-metacharacters",
            json!({"title": "a<b & c>d",
                   "shapes": [
                       {"type": "text", "x": 0.0, "y": 5.0, "content": "1 < 2 & 3 > 2"},
                       {"type": "rect", "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0,
                        "id": "q\"uote", "class": "a&b"}]}),
        ),
        (
            "many-shapes",
            json!({"viewBox": [0.0, 0.0, 8.0, 8.0],
                   "shapes": [
                       {"type": "rect", "x": 0.0, "y": 0.0, "width": 2.0, "height": 2.0, "id": "a"},
                       {"type": "rect", "x": 2.0, "y": 2.0, "width": 2.0, "height": 2.0, "id": "b"},
                       {"type": "rect", "x": 4.0, "y": 4.0, "width": 2.0, "height": 2.0},
                       {"type": "circle", "cx": 6.0, "cy": 6.0, "r": 1.0, "class": "c1"},
                       {"type": "text", "x": 0.0, "y": 7.0, "content": "labels"}]}),
        ),
        (
            "fractional-geometry",
            json!({"viewBox": [0.5, 0.25, 10.75, 5.125],
                   "shapes": [{"type": "rect", "x": 0.1, "y": 0.2, "width": 3.35, "height": 4.0}]}),
        ),
    ]
}

#[test]
fn corpus_count_is_at_least_ten() {
    // The plan's acceptance: round-trip empty-diff on a spec corpus >= 10.
    assert!(corpus().len() >= 10, "corpus must have >= 10 specs");
}

#[test]
fn round_trip_is_empty_diff_on_the_whole_corpus() {
    for (name, params) in corpus() {
        // Given: the spec and its INDEPENDENT expected tree.
        let input = spec(params);
        let expected = SvgSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: spec must parse: {error}"))
            .canonical_tree();
        // When: generated then verified through the adapter's verify
        // (extract + M5 diff under the svg profile + usvg strict parse).
        let bytes = svg::generate(&input).unwrap_or_else(|error| panic!("{name}: {error}"));
        let verdict = svg::verify(&expected, &bytes)
            .unwrap_or_else(|error| panic!("{name}: verify errored: {error}"));
        // Then: PASS (empty diff + strict parse) on every corpus row.
        assert!(
            verdict.is_pass(),
            "{name}: round trip failed:\n{}",
            match &verdict {
                RoundTripVerdict::Fail(diff) => diff.to_string(),
                RoundTripVerdict::Pass => String::new(),
            }
        );
    }
}

#[test]
fn round_trip_passes_through_the_executor_dispatch_path() {
    // The same gate, dispatched through the M6 executor (the registration
    // is live, not just the free functions).
    let registry = harness_media::executor::registry();
    for (name, params) in corpus() {
        let input = spec(params);
        let expected = SvgSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .canonical_tree();
        let generated = registry
            .dispatch(FormatId::Svg, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(
                FormatId::Svg,
                OpRequest::Verify {
                    expected: &expected,
                    bytes,
                },
            )
            .unwrap_or_else(|error| panic!("{name}: dispatch verify: {error}"));
        let OpOutput::Verify(verdict) = verified.inner() else {
            panic!("{name}: expected Verify output");
        };
        assert!(verdict.is_pass(), "{name}: executor-path round trip failed");
    }
}

#[test]
fn mutated_expected_tree_fails_with_the_exact_pointer() {
    // PROVENANCE (plan M5): the gate must be non-vacuous. Mutating the
    // independent expected tree at a known pointer must fail verification
    // naming that exact pointer.
    let params = json!({"viewBox": [0.0, 0.0, 10.0, 10.0],
                        "shapes": [{"type": "rect", "x": 1.0, "y": 1.0, "width": 2.0, "height": 2.0}]});
    let input = spec(params);
    let mut expected = SvgSpec::from_generation(&input)
        .expect("spec parses")
        .canonical_tree();
    let bytes = svg::generate(&input).expect("generate");
    // When: the expected census count is mutated.
    expected["census"]["rect"] = json!(2);
    let verdict = svg::verify(&expected, &bytes).expect("verify runs");
    // Then: Fail with the exact pointer.
    match verdict {
        RoundTripVerdict::Fail(diff) => {
            assert_eq!(diff.changed.len(), 1, "{}", diff);
            assert_eq!(diff.changed[0].path, "/census/rect");
        }
        RoundTripVerdict::Pass => panic!("mutated expectation must not pass"),
    }
}

#[test]
fn structural_violation_fails_the_usvg_strict_parse_stage() {
    // The usvg stage-1 hook: bytes that quick-xml parses but usvg
    // rejects must fail verify typed (never a silent pass). A zero-size
    // (width=0) svg is invalid per usvg's strict size handling.
    let bytes = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\"/>";
    let error = svg::verify(&json!({}), bytes).expect_err("zero-size must be refused by usvg");
    assert!(
        matches!(error, harness_media::error::MediaError::ParseFailure),
        "got {error:?}"
    );
}

#[test]
fn spec_tree_projection_is_independent_of_extraction() {
    // The independent projection is a pure function of the SPEC: same
    // spec -> same tree, and a spec change moves the tree (no hidden
    // coupling to the writer).
    let params = json!({"viewBox": [0.0, 0.0, 4.0, 4.0], "title": "X",
                        "shapes": [{"type": "circle", "cx": 1.0, "cy": 1.0, "r": 1.0, "id": "c"}]});
    let input = spec(params);
    let first = SvgSpec::from_generation(&input)
        .expect("spec")
        .canonical_tree();
    let second = SvgSpec::from_generation(&input)
        .expect("spec")
        .canonical_tree();
    assert_eq!(first, second, "deterministic projection");
    let changed = spec(json!({"viewBox": [0.0, 0.0, 8.0, 8.0], "title": "X",
                              "shapes": [{"type": "circle", "cx": 1.0, "cy": 1.0, "r": 1.0, "id": "c"}]}));
    let other = SvgSpec::from_generation(&changed)
        .expect("spec")
        .canonical_tree();
    assert_ne!(first, other, "spec changes move the projected tree");
}

#[test]
fn profile_is_float_eps_with_no_unordered_escapes() {
    // The svg diff profile: float epsilon only; no unordered pointers
    // (spec-driven writes are ordered-exact), no ignored subtrees.
    let profile = svg::profile();
    assert!(profile.float_epsilon > 0.0);
    assert!(profile.unordered_at.is_empty());
    assert!(profile.ignored_at.is_empty());
}
