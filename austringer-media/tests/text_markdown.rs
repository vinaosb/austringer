//! M30 markdown lane: comrak (CommonMark 0.31.2 + GFM) census extract,
//! canonical spec->markdown generation, strict reparse + M5 diff verify,
//! and the deterministic md->html projection. Hermetic.

use std::sync::Arc;
use std::time::Duration;

use austringer_media::adapter::text::markdown::{self, MdSpec};
use austringer_media::error::MediaError;
use austringer_media::executor::{OpOutput, OpRequest};
use austringer_media::extract::ExtractBudgets;
use austringer_media::format::FormatId;
use austringer_media::generate::GenerationSpec;
use austringer_media::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};
use serde_json::{Value, json};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Markdown,
        params,
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}

/// The spec corpus (>= 10 per the plan acceptance): every block kind,
/// heading levels, unicode, metacharacter text, multi-row tables.
fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        ("empty", json!({"blocks": []})),
        (
            "single-heading",
            json!({"blocks": [{"type": "heading", "level": 1, "text": "T"}]}),
        ),
        (
            "heading-levels",
            json!({"blocks": [
                {"type": "heading", "level": 1, "text": "one"},
                {"type": "heading", "level": 3, "text": "two"},
                {"type": "heading", "level": 6, "text": "six"}]}),
        ),
        (
            "paragraphs",
            json!({"blocks": [
            {"type": "paragraph", "text": "first body"},
            {"type": "paragraph", "text": "second body"}]}),
        ),
        (
            "code-fence-language",
            json!({"blocks": [{"type": "code_fence", "language": "rust", "code": "fn main() {}"}]}),
        ),
        (
            "code-fence-bare",
            json!({"blocks": [{"type": "code_fence", "code": "bare\ncode"}]}),
        ),
        (
            "bullet-list",
            json!({"blocks": [{"type": "list", "ordered": false, "items": ["a", "b"]}]}),
        ),
        (
            "ordered-list",
            json!({"blocks": [{"type": "list", "ordered": true, "items": ["x", "y"]}]}),
        ),
        (
            "gfm-table",
            json!({"blocks": [{"type": "table", "header": ["H1", "H2"], "rows": [["a", "b"], ["c", "d"]]}]}),
        ),
        (
            "mixed-document",
            json!({"blocks": [
                {"type": "heading", "level": 1, "text": "Doc"},
                {"type": "paragraph", "text": "intro"},
                {"type": "code_fence", "language": "toml", "code": "a = 1"},
                {"type": "list", "ordered": false, "items": ["m", "n"]},
                {"type": "table", "header": ["k"], "rows": [["v1"], ["v2"]]}]}),
        ),
        (
            "unicode",
            json!({"blocks": [
                {"type": "heading", "level": 2, "text": "héading — 日本語"},
                {"type": "paragraph", "text": "ünïcode bödý ✓"}]}),
        ),
        (
            "metacharacters",
            json!({"blocks": [
                {"type": "paragraph", "text": "a *b* c"},
                {"type": "paragraph", "text": "# not a heading"},
                {"type": "paragraph", "text": "1 < 2 & 3 > 2"}]}),
        ),
        (
            "empty-code-fence",
            json!({"blocks": [{"type": "code_fence", "code": ""}]}),
        ),
    ]
}

#[test]
fn corpus_count_is_at_least_ten() {
    assert!(corpus().len() >= 10, "corpus must have >= 10 specs");
}

#[test]
fn round_trip_is_empty_diff_on_the_whole_corpus() {
    for (name, params) in corpus() {
        // Given: the spec and its INDEPENDENT expected tree.
        let input = spec(params);
        let expected = MdSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: spec must parse: {error}"))
            .canonical_tree();
        // When: generated then verified (comrak reparse + M5 diff).
        let bytes =
            markdown::generate(&input).unwrap_or_else(|error| panic!("{name}: generate: {error}"));
        let verdict = markdown::verify(&expected, &bytes)
            .unwrap_or_else(|error| panic!("{name}: verify errored: {error}"));
        // Then: PASS on every corpus row.
        assert!(
            verdict.is_pass(),
            "{name}: round trip failed:\n{}",
            match &verdict {
                austringer_media::diff::RoundTripVerdict::Fail(diff) => diff.to_string(),
                austringer_media::diff::RoundTripVerdict::Pass => String::new(),
            }
        );
    }
}

#[test]
fn round_trip_passes_through_the_executor_dispatch_path() {
    let registry = austringer_media::executor::registry();
    for (name, params) in corpus() {
        let input = spec(params);
        let expected = MdSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .canonical_tree();
        let generated = registry
            .dispatch(FormatId::Markdown, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(
                FormatId::Markdown,
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
    // PROVENANCE: the gate must be non-vacuous.
    let input = spec(json!({"blocks": [{"type": "heading", "level": 1, "text": "T"}]}));
    let mut expected = MdSpec::from_generation(&input)
        .expect("spec")
        .canonical_tree();
    let bytes = markdown::generate(&input).expect("generate");
    expected["headings"][0]["level"] = json!(2);
    match markdown::verify(&expected, &bytes).expect("verify runs") {
        austringer_media::diff::RoundTripVerdict::Fail(diff) => {
            assert_eq!(diff.changed.len(), 1, "{}", diff);
            assert_eq!(diff.changed[0].path, "/headings/0/level");
        }
        austringer_media::diff::RoundTripVerdict::Pass => {
            panic!("mutated expectation must not pass")
        }
    }
}

#[test]
fn minimal_fixture_extracts_the_pinned_census() {
    // Given: the M2 probe fixture (heading + bullet list + paragraph).
    // When/Then: the census is the pinned tree.
    let extracted =
        markdown::extract(&fixture("markdown.md"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "code_blocks": 0,
            "code_languages": {},
            "headings": [{"level": 1, "text": "Austringer fixture"}],
            "lists": 1,
            "paragraphs": 3,
            "tables": 0,
        })
    );
}

#[test]
fn rich_fixture_extracts_the_pinned_census() {
    let extracted =
        markdown::extract(&fixture("markdown-doc.md"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "code_blocks": 2,
            "code_languages": {"": 1, "rust": 1},
            "headings": [
                {"level": 1, "text": "Title"},
                {"level": 2, "text": "Section A"},
                {"level": 3, "text": "Subsection"},
            ],
            "lists": 2,
            "paragraphs": 6,
            "tables": 1,
        })
    );
    assert!(extracted.text_projection.contains("Title"));
    assert!(extracted.text_projection.contains("2 code_blocks"));
    assert!(extracted.text_projection.len() <= markdown::TEXT_PROJECTION_MAX_CHARS);
}

#[test]
fn setext_and_atx_headings_extract_identically() {
    // Dialect bleed pinned: setext and ATX spellings of the same heading
    // land on the SAME AST census (strict CommonMark equivalence, not a
    // byte comparison).
    let setext = markdown::extract(b"Title\n=====\n\nbody\n", ExtractBudgets::default())
        .expect("setext extracts");
    let atx =
        markdown::extract(b"# Title\n\nbody\n", ExtractBudgets::default()).expect("atx extracts");
    assert_eq!(setext.tree, atx.tree);
}

#[test]
fn gfm_table_in_raw_input_extracts() {
    let extracted = markdown::extract(
        b"| A | B |\n| -- | -- |\n| 1 | 2 |\n",
        ExtractBudgets::default(),
    )
    .expect("gfm table extracts");
    assert_eq!(extracted.tree["tables"], json!(1));
}

#[test]
fn any_utf8_text_extracts_markdown_is_total() {
    // CommonMark is total over text input: there is no malformed
    // markdown, only bytes that are not utf8.
    let extracted =
        markdown::extract(b"just plain text", ExtractBudgets::default()).expect("extracts");
    assert_eq!(extracted.tree["paragraphs"], json!(1));
}

#[test]
fn non_utf8_is_typed_parse_failure() {
    let error = markdown::extract(&[0xff, 0xfe, 0x23], ExtractBudgets::default())
        .expect_err("non-utf8 must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    let bytes = fixture("markdown-doc.md");
    let zero_deadline = ExtractBudgets {
        deadline: Duration::ZERO,
        ..ExtractBudgets::default()
    };
    let error = markdown::extract(&bytes, zero_deadline).expect_err("zero deadline");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
    let tiny = ExtractBudgets {
        max_total_bytes: 16,
        ..ExtractBudgets::default()
    };
    let error = markdown::extract(&bytes, tiny).expect_err("tiny cap");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn md_to_html_projection_is_deterministic() {
    // The plan acceptance: md->html projection deterministic (byte-stable
    // across runs, GFM-aware).
    let bytes = fixture("markdown-doc.md");
    let first = markdown::render_html(&bytes, ExtractBudgets::default()).expect("render 1");
    let second = markdown::render_html(&bytes, ExtractBudgets::default()).expect("render 2");
    assert_eq!(first, second, "deterministic render");
    let html = String::from_utf8(first).expect("utf8 html");
    assert!(html.contains("<table"), "GFM table renders: {html}");
    assert!(html.contains("<h2"), "headings render: {html}");
}

#[test]
fn generation_is_byte_deterministic() {
    let input = spec(corpus()[9].1.clone());
    let first = markdown::generate(&input).expect("generate 1");
    let second = markdown::generate(&input).expect("generate 2");
    assert_eq!(first, second);
}

#[test]
fn invalid_specs_are_typed() {
    let invalid = [
        json!({"blocks": [{"type": "heading", "level": 0, "text": "x"}]}),
        json!({"blocks": [{"type": "heading", "level": 7, "text": "x"}]}),
        json!({"blocks": [{"type": "paragraph", "text": ""}]}),
        json!({"blocks": [{"type": "paragraph", "text": "two  spaces"}]}),
        json!({"blocks": [{"type": "paragraph", "text": "line\nbreak"}]}),
        json!({"blocks": [{"type": "list", "ordered": false, "items": []}]}),
        json!({"blocks": [{"type": "table", "header": ["A"], "rows": [["x", "y"]]}]}),
        json!({"blocks": [{"type": "table", "header": [], "rows": []}]}),
        json!({"blocks": [{"type": "code_fence", "language": "has space", "code": "x"}]}),
    ];
    for params in invalid {
        let error = markdown::generate(&spec(params)).expect_err("must be InvalidSpec");
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
    // Wrong format is refused before params are even read.
    let foreign = GenerationSpec {
        format: FormatId::Html,
        params: json!({"blocks": []}),
    };
    assert!(matches!(
        markdown::generate(&foreign),
        Err(MediaError::InvalidSpec)
    ));
}

#[test]
fn projector_is_a_thin_wrap_over_the_extractor() {
    let bytes = fixture("markdown-doc.md");
    let extracted = markdown::extract(&bytes, ExtractBudgets::default()).expect("extract");
    let context = ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    let projected = markdown::MarkdownProjector
        .project(&bytes, &context)
        .expect("project");
    assert_eq!(projected, extracted.text_projection);

    let mut registry = ProjectionRegistry::new();
    registry
        .register(FormatId::Markdown, Arc::new(markdown::MarkdownProjector))
        .expect("register");
    let mismatch = registry
        .project(
            &FormatId::Markdown,
            austringer_core::event::ProjectionKind::Transcript,
            &bytes,
            &context,
        )
        .expect_err("kind mismatch is typed");
    assert!(
        matches!(mismatch, MediaError::NoProjector),
        "got {mismatch:?}"
    );
}
