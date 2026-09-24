//! M30 html5 lane: html5gum tokenizer census extract, maud generation
//! (self-contained single file), and the html5ever/html5gum reparse
//! verify. HTML5 is lossy as interchange (no pagination/JS execution) -
//! generation is honest structural HTML only. Hermetic.

use std::sync::Arc;

use harness_media::adapter::text::html::{self, HtmlSpec};
use harness_media::error::MediaError;
use harness_media::executor::{OpRequest, OpOutput};
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use harness_media::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};
use serde_json::{json, Value};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec { format: FormatId::Html, params }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}

fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        ("empty-no-title", json!({"blocks": []})),
        ("title-only", json!({"title": "T", "blocks": []})),
        (
            "heading-levels",
            json!({"blocks": [
                {"type": "heading", "level": 1, "text": "one"},
                {"type": "heading", "level": 2, "text": "two"},
                {"type": "heading", "level": 6, "text": "six"}]}),
        ),
        ("paragraphs", json!({"blocks": [
            {"type": "paragraph", "text": "first"},
            {"type": "paragraph", "text": "second"}]})),
        ("bullet-list", json!({"blocks": [{"type": "list", "ordered": false, "items": ["a", "b"]}]})),
        ("ordered-list", json!({"blocks": [{"type": "list", "ordered": true, "items": ["x"]}]})),
        ("code", json!({"blocks": [{"type": "code", "text": "let x = 1;"}]})),
        (
            "table",
            json!({"blocks": [{"type": "table", "header": ["H1", "H2"], "rows": [["a", "b"]]}]}),
        ),
        (
            "mixed-document",
            json!({"title": "Doc", "blocks": [
                {"type": "heading", "level": 1, "text": "Main"},
                {"type": "paragraph", "text": "intro"},
                {"type": "list", "ordered": false, "items": ["m", "n"]},
                {"type": "code", "text": "x = 1"},
                {"type": "table", "header": ["k"], "rows": [["v"]]}]}),
        ),
        (
            "unicode",
            json!({"title": "Ünïcode", "blocks": [
                {"type": "paragraph", "text": "日本語 ✓ — héllo"}]}),
        ),
        (
            "escaping",
            json!({"blocks": [
                {"type": "paragraph", "text": "1 < 2 & 3 > 2"},
                {"type": "code", "text": "<script>alert(1)</script>"}]}),
        ),
        (
            "empty-cells",
            json!({"blocks": [{"type": "table", "header": ["A", "B"], "rows": [["", "x"]]}]}),
        ),
    ]
}

#[test]
fn corpus_count_is_at_least_ten() {
    assert!(corpus().len() >= 10);
}

#[test]
fn round_trip_is_empty_diff_on_the_whole_corpus() {
    for (name, params) in corpus() {
        let input = spec(params);
        let expected = HtmlSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: spec must parse: {error}"))
            .canonical_tree();
        let bytes = html::generate(&input)
            .unwrap_or_else(|error| panic!("{name}: generate: {error}"));
        let verdict = html::verify(&expected, &bytes)
            .unwrap_or_else(|error| panic!("{name}: verify errored: {error}"));
        assert!(
            verdict.is_pass(),
            "{name}: round trip failed:\n{}",
            match &verdict {
                harness_media::diff::RoundTripVerdict::Fail(diff) => diff.to_string(),
                harness_media::diff::RoundTripVerdict::Pass => String::new(),
            }
        );
    }
}

#[test]
fn round_trip_passes_through_the_executor_dispatch_path() {
    let registry = harness_media::executor::registry();
    for (name, params) in corpus() {
        let input = spec(params);
        let expected = HtmlSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .canonical_tree();
        let generated = registry
            .dispatch(FormatId::Html, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(FormatId::Html, OpRequest::Verify { expected: &expected, bytes })
            .unwrap_or_else(|error| panic!("{name}: dispatch verify: {error}"));
        let OpOutput::Verify(verdict) = verified.inner() else {
            panic!("{name}: expected Verify output");
        };
        assert!(verdict.is_pass(), "{name}: executor-path round trip failed");
    }
}

#[test]
fn minimal_fixture_extracts_the_pinned_census() {
    let extracted =
        html::extract(&fixture("html.html"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "census": {"body": 1, "head": 1, "html": 1, "p": 1, "title": 1},
            "doctype": true,
            "text": "fixture hello",
            "title": "fixture",
        })
    );
}

#[test]
fn rich_fixture_extracts_the_pinned_census() {
    let extracted =
        html::extract(&fixture("html-doc.html"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "census": {
                "body": 1, "h1": 1, "head": 1, "html": 1, "li": 2, "p": 1,
                "table": 1, "tbody": 1, "td": 2, "th": 2, "thead": 1,
                "title": 1, "tr": 2, "ul": 1,
            },
            "doctype": true,
            "text": "doc fixture Main intro one two k v a 1",
            "title": "doc fixture",
        })
    );
}

#[test]
fn generated_documents_are_self_contained() {
    // The digest interchange note: generation emits a self-contained
    // single file - no external assets, no scripts, no linked styles.
    for (_, params) in corpus() {
        let bytes = html::generate(&spec(params)).expect("generate");
        let text = String::from_utf8(bytes).expect("utf8");
        let lower = text.to_ascii_lowercase();
        assert!(!lower.contains("<script"), "no scripts: {lower}");
        assert!(!lower.contains("src="), "no external sources: {lower}");
        assert!(!lower.contains("href="), "no links: {lower}");
        assert!(!lower.contains("<link"), "no stylesheets: {lower}");
        assert!(lower.starts_with("<!doctype html>"), "html5 doctype: {lower}");
        assert!(lower.contains("<meta charset=\"utf-8\">"), "charset declared: {lower}");
    }
}

#[test]
fn mutated_expected_tree_fails_with_the_exact_pointer() {
    let input = spec(json!({"blocks": [{"type": "paragraph", "text": "x"}]}));
    let mut expected = HtmlSpec::from_generation(&input).expect("spec").canonical_tree();
    let bytes = html::generate(&input).expect("generate");
    expected["census"]["p"] = json!(2);
    match html::verify(&expected, &bytes).expect("verify runs") {
        harness_media::diff::RoundTripVerdict::Fail(diff) => {
            assert_eq!(diff.changed.len(), 1, "{}", diff);
            assert_eq!(diff.changed[0].path, "/census/p");
        }
        harness_media::diff::RoundTripVerdict::Pass => panic!("mutated expectation must not pass"),
    }
}

#[test]
fn fragment_without_doctype_extracts_honestly() {
    // HTML5 parsing has no error concept (error recovery by design); a
    // doctype-less fragment is a census fact, never a failure.
    let extracted =
        html::extract(b"<p>x</p>", ExtractBudgets::default()).expect("extracts");
    assert_eq!(extracted.tree["doctype"], json!(false));
    assert_eq!(extracted.tree["census"], json!({"p": 1}));
}

#[test]
fn non_utf8_is_typed_parse_failure() {
    let error = html::extract(&[0x3c, 0xff, 0x3e], ExtractBudgets::default())
        .expect_err("non-utf8 must refuse (utf8-only lane, disclosed)");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    let bytes = fixture("html-doc.html");
    let zero = ExtractBudgets {
        deadline: std::time::Duration::ZERO,
        ..ExtractBudgets::default()
    };
    let error = html::extract(&bytes, zero).expect_err("zero deadline");
    assert!(matches!(error, MediaError::ExtractTooLarge), "got {error:?}");
    let tiny = ExtractBudgets { max_total_bytes: 8, ..ExtractBudgets::default() };
    let error = html::extract(&bytes, tiny).expect_err("tiny cap");
    assert!(matches!(error, MediaError::ExtractTooLarge), "got {error:?}");
}

#[test]
fn generation_is_byte_deterministic() {
    let input = spec(corpus()[8].1.clone());
    assert_eq!(html::generate(&input).expect("1"), html::generate(&input).expect("2"));
}

#[test]
fn invalid_specs_are_typed() {
    let invalid = [
        json!({"blocks": [{"type": "heading", "level": 0, "text": "x"}]}),
        json!({"blocks": [{"type": "heading", "level": 7, "text": "x"}]}),
        json!({"blocks": [{"type": "paragraph", "text": ""}]}),
        json!({"blocks": [{"type": "paragraph", "text": "two  spaces"}]}),
        json!({"blocks": [{"type": "list", "ordered": true, "items": []}]}),
        json!({"blocks": [{"type": "table", "header": ["A"], "rows": [["x", "y"]]}]}),
        json!({"title": "multi\nline", "blocks": []}),
    ];
    for params in invalid {
        let error = html::generate(&spec(params)).expect_err("must be InvalidSpec");
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
}

#[test]
fn projector_is_a_thin_wrap_over_the_extractor() {
    let bytes = fixture("html-doc.html");
    let extracted = html::extract(&bytes, ExtractBudgets::default()).expect("extract");
    let context = ProjectionContext { budgets: ExtractBudgets::default(), name_hint: None };
    let projected = html::HtmlProjector.project(&bytes, &context).expect("project");
    assert_eq!(projected, extracted.text_projection);
    assert!(projected.len() <= html::TEXT_PROJECTION_MAX_CHARS);

    let mut registry = ProjectionRegistry::new();
    registry
        .register(FormatId::Html, Arc::new(html::HtmlProjector))
        .expect("register");
    let duplicate = registry
        .register(FormatId::Html, Arc::new(html::HtmlProjector))
        .expect_err("duplicate refused");
    assert!(matches!(
        duplicate,
        harness_media::projection::ProjectionWiringError::DuplicateProjector { format }
            if format == FormatId::Html
    ));
}
