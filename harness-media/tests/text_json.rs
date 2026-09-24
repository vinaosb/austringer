//! M30 json/jsonl lane: RFC 8259 serde_json canonical (sorted-key) read/
//! write, the JSONL lane, and the M5 reparse-diff verify. Hermetic.

use std::sync::Arc;

use harness_media::adapter::text::json::{self, JsonSpec};
use harness_media::error::MediaError;
use harness_media::executor::{OpRequest, OpOutput};
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use harness_media::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};
use serde_json::{json, Value};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec { format: FormatId::Json, params }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}

fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        ("scalar-int", json!({"value": 42})),
        ("scalar-string", json!({"value": "text"})),
        ("null", json!({"value": null})),
        ("bool", json!({"value": true})),
        ("float", json!({"value": 0.1})),
        ("empty-object", json!({"value": {}})),
        ("nested", json!({"value": {"a": {"b": [1, 2, {"c": null}]}, "d": false}})),
        ("array", json!({"value": [1, "two", false, null]})),
        ("unicode", json!({"value": {"k": "日本語 ✓ é"}})),
        (
            "mixed-doc",
            json!({"value": {"name": "harness", "count": 34, "tags": ["a", "b"]}}),
        ),
        (
            "jsonl",
            json!({"rows": [{"n": 1}, {"n": 2}, {"n": 3}]}),
        ),
        ("jsonl-scalars", json!({"rows": [1, "x", null]})),
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
        let expected = JsonSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: spec must parse: {error}"))
            .canonical_tree();
        let bytes = json::generate(&input)
            .unwrap_or_else(|error| panic!("{name}: generate: {error}"));
        let verdict = json::verify(&expected, &bytes)
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
        let expected = JsonSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .canonical_tree();
        let generated = registry
            .dispatch(FormatId::Json, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(FormatId::Json, OpRequest::Verify { expected: &expected, bytes })
            .unwrap_or_else(|error| panic!("{name}: dispatch verify: {error}"));
        let OpOutput::Verify(verdict) = verified.inner() else {
            panic!("{name}: expected Verify output");
        };
        assert!(verdict.is_pass(), "{name}: executor-path round trip failed");
    }
}

#[test]
fn canonical_write_sorts_keys_and_is_byte_exact() {
    // Canonical R/W: keys sorted, compact, trailing newline (determinism
    // pinned at byte level, not just tree equality).
    let input = spec(json!({"value": {"b": 1, "a": [true, null]}}));
    let bytes = json::generate(&input).expect("generate");
    assert_eq!(bytes, b"{\"a\":[true,null],\"b\":1}\n");
}

#[test]
fn jsonl_write_is_one_value_per_line() {
    let input = spec(json!({"rows": [{"z": 1, "a": 2}, "tail"]}));
    let bytes = json::generate(&input).expect("generate");
    assert_eq!(bytes, b"{\"a\":2,\"z\":1}\n\"tail\"\n");
}

#[test]
fn fixture_extract_tree_is_the_value() {
    let extracted = json::extract(&fixture("json.json"), ExtractBudgets::default())
        .expect("extract");
    assert_eq!(extracted.tree, json!({"count": 34, "name": "harness"}));
}

#[test]
fn jsonl_fixture_extracts_the_tagged_tree() {
    let extracted = json::extract(&fixture("jsonl-doc.jsonl"), ExtractBudgets::default())
        .expect("extract");
    assert_eq!(
        extracted.tree,
        json!({"jsonl": true, "rows": [{"n": 1}, {"n": 2}, {"n": 3}]})
    );
    assert!(extracted.text_projection.contains("3 rows"));
}

#[test]
fn unsorted_input_canonicalizes_through_extract_then_write() {
    // Duplicate keys: last-wins (serde_json; RFC 8259 permits) - the
    // canonical form is what round trips.
    let extracted =
        json::extract(b"{\"b\":1,\"a\":2,\"b\":3}", ExtractBudgets::default())
            .expect("extract");
    assert_eq!(extracted.tree, json!({"a": 2, "b": 3}));
}

#[test]
fn malformed_input_is_typed_parse_failure() {
    for bytes in [
        &b"{oops"[..],
        b"1 2",
        b"",
        b"{\"a\":1}\n{bad",
        b"[1, 2",
    ] {
        let error = json::extract(bytes, ExtractBudgets::default())
            .expect_err("must be ParseFailure");
        assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
    }
}

#[test]
fn non_utf8_is_typed_parse_failure() {
    let error =
        json::extract(&[0xff, 0xfe], ExtractBudgets::default()).expect_err("must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    let bytes = fixture("jsonl-doc.jsonl");
    let tiny = ExtractBudgets { max_total_bytes: 4, ..ExtractBudgets::default() };
    let error = json::extract(&bytes, tiny).expect_err("tiny cap");
    assert!(matches!(error, MediaError::ExtractTooLarge), "got {error:?}");
}

#[test]
fn spec_tree_projection_is_independent_of_extraction() {
    let params = json!({"value": {"a": 1}});
    let parsed = JsonSpec::from_generation(&spec(params)).expect("spec");
    assert_eq!(parsed.canonical_tree(), json!({"a": 1}));
    // A spec change moves the tree (no coupling to the writer).
    let other = JsonSpec::from_generation(&spec(json!({"value": {"a": 2}}))).expect("spec");
    assert_ne!(parsed.canonical_tree(), other.canonical_tree());
}

#[test]
fn generation_is_byte_deterministic() {
    let input = spec(corpus()[6].1.clone());
    assert_eq!(json::generate(&input).expect("1"), json::generate(&input).expect("2"));
}

#[test]
fn invalid_specs_are_typed() {
    let invalid = [
        json!({}),
        json!({"value": {"a": 1}, "rows": [1]}),
        json!({"rows": []}),
        json!({"rows": "not-an-array"}),
    ];
    for params in invalid {
        let error = json::generate(&spec(params)).expect_err("must be InvalidSpec");
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
    let foreign = GenerationSpec { format: FormatId::Csv, params: json!({"value": 1}) };
    assert!(matches!(json::generate(&foreign), Err(MediaError::InvalidSpec)));
}

#[test]
fn projector_is_a_thin_wrap_over_the_extractor() {
    let bytes = fixture("jsonl-doc.jsonl");
    let extracted = json::extract(&bytes, ExtractBudgets::default()).expect("extract");
    let context = ProjectionContext { budgets: ExtractBudgets::default(), name_hint: None };
    let projected = json::JsonProjector.project(&bytes, &context).expect("project");
    assert_eq!(projected, extracted.text_projection);

    let mut registry = ProjectionRegistry::new();
    registry
        .register(FormatId::Json, Arc::new(json::JsonProjector))
        .expect("register");
    let unregistered = registry
        .project(&FormatId::Toml, harness_core::event::ProjectionKind::TextProjection, b"x = 1", &context)
        .expect_err("no projector registered for toml here");
    assert!(matches!(unregistered, MediaError::NoProjector), "got {unregistered:?}");
}
