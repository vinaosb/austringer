//! M30 toml lane: TOML 1.0 via the toml crate - parse-tree extract (with
//! datetime-as-string normalization), canonical writer, M5 reparse-diff
//! verify. Hermetic.

use std::sync::Arc;

use austringer_media::adapter::text::toml::{self, TomlSpec};
use austringer_media::error::MediaError;
use austringer_media::executor::{OpOutput, OpRequest};
use austringer_media::extract::ExtractBudgets;
use austringer_media::format::FormatId;
use austringer_media::generate::GenerationSpec;
use austringer_media::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};
use serde_json::{Value, json};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Toml,
        params,
    }
}

fn doc(value: Value) -> Value {
    json!({"value": value})
}

fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        ("empty-table", doc(json!({}))),
        ("simple-kv", doc(json!({"name": "austringer", "count": 34}))),
        ("booleans", doc(json!({"on": true, "off": false}))),
        ("floats", doc(json!({"ratio": 0.5, "neg": -1.25}))),
        (
            "nested-tables",
            doc(json!({"server": {"host": "localhost", "port": 8080}})),
        ),
        ("array-of-ints", doc(json!({"ports": [8000, 8001]}))),
        (
            "array-of-tables",
            doc(json!({"items": [{"b": 1}, {"b": 2}]})),
        ),
        ("empty-array", doc(json!({"none": []}))),
        (
            "unicode-strings",
            doc(json!({"k": "日本語 ✓ é", "esc": "line1\nline2\ttab"})),
        ),
        (
            "mixed",
            doc(json!({"title": "doc", "owner": {"active": true}, "list": ["a", "b"]})),
        ),
        (
            "negative-int",
            doc(json!({"delta": -9223372036854775808_i64})),
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
        let expected = TomlSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: spec must parse: {error}"))
            .canonical_tree();
        let bytes =
            toml::generate(&input).unwrap_or_else(|error| panic!("{name}: generate: {error}"));
        let verdict = toml::verify(&expected, &bytes)
            .unwrap_or_else(|error| panic!("{name}: verify errored: {error}"));
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
        let expected = TomlSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .canonical_tree();
        let generated = registry
            .dispatch(FormatId::Toml, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(
                FormatId::Toml,
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
fn minimal_fixture_extracts_the_pinned_tree() {
    let extracted =
        toml::extract(&fixture("toml.toml"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({"server": {"host": "localhost", "port": 8080}})
    );
}

#[test]
fn rich_fixture_extracts_datetimes_as_strings() {
    let extracted =
        toml::extract(&fixture("toml-doc.toml"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "title": "doc",
            "owner": {"name": "austringer", "active": true},
            "server": {
                "ports": [8000, 8001],
                "start": "2026-01-01T00:00:00Z",
                "ratio": 0.5,
            },
        })
    );
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}

#[test]
fn invalid_toml_is_typed_parse_failure() {
    for bytes in [
        &b"[unclosed\n"[..],
        b"x = ",
        b"= 5",
        b"[a]\n[a]\n duplication = 1\n[a]",
    ] {
        let error =
            toml::extract(bytes, ExtractBudgets::default()).expect_err("must be ParseFailure");
        assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
    }
}

#[test]
fn non_utf8_is_typed_parse_failure() {
    let error = toml::extract(&[0xff, 0x3d, 0x20, 0x31], ExtractBudgets::default())
        .expect_err("must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    let bytes = fixture("toml-doc.toml");
    let tiny = ExtractBudgets {
        max_total_bytes: 4,
        ..ExtractBudgets::default()
    };
    let error = toml::extract(&bytes, tiny).expect_err("tiny cap");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn spec_tree_projection_is_independent_of_extraction() {
    let parsed = TomlSpec::from_generation(&spec(doc(json!({"a": 1})))).expect("spec");
    assert_eq!(parsed.canonical_tree(), json!({"a": 1}));
    let other = TomlSpec::from_generation(&spec(doc(json!({"a": 2})))).expect("spec");
    assert_ne!(parsed.canonical_tree(), other.canonical_tree());
}

#[test]
fn generation_is_byte_deterministic_with_sorted_keys() {
    let input = spec(doc(json!({"b": 1, "a": 2})));
    let first = toml::generate(&input).expect("generate 1");
    let second = toml::generate(&input).expect("generate 2");
    assert_eq!(first, second);
    assert_eq!(first, b"a = 2\nb = 1\n");
}

#[test]
fn generation_refuses_unrepresentable_values() {
    // TOML 1.0: root must be a table, no null, homogeneous arrays only -
    // each is the typed InvalidSpec, never a mangled write.
    let invalid = [
        doc(json!(42)),
        doc(json!([1, 2])),
        doc(json!({"a": null})),
        doc(json!({"a": [1, "b"]})),
        doc(json!({"nested": {"deep": null}})),
    ];
    for params in invalid {
        let error = toml::generate(&spec(params)).expect_err("must be InvalidSpec");
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
    let foreign = GenerationSpec {
        format: FormatId::Json,
        params: doc(json!({})),
    };
    assert!(matches!(
        toml::generate(&foreign),
        Err(MediaError::InvalidSpec)
    ));
}

#[test]
fn projector_is_a_thin_wrap_over_the_extractor() {
    let bytes = fixture("toml-doc.toml");
    let extracted = toml::extract(&bytes, ExtractBudgets::default()).expect("extract");
    let context = ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    let projected = toml::TomlProjector
        .project(&bytes, &context)
        .expect("project");
    assert_eq!(projected, extracted.text_projection);
    assert!(projected.len() <= toml::TEXT_PROJECTION_MAX_CHARS);

    let mut registry = ProjectionRegistry::new();
    registry
        .register(FormatId::Toml, Arc::new(toml::TomlProjector))
        .expect("register");
    let mismatch = registry
        .project(
            &FormatId::Toml,
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
