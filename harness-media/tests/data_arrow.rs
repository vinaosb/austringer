//! M22 arrow-ipc lane: the same crate's ipc module (file format, magic
//! `ARROW1`). Extract = Schema + record-batch census + bounded head
//! sample; generate = a spec-driven record-batch write; verify = re-read,
//! arrow `Schema::eq` folded into the M5 structural diff. Hermetic.

use harness_media::adapter::data::arrow;
use harness_media::adapter::data::TabularSpec;
use harness_media::error::MediaError;
use harness_media::executor::{OpRequest, OpOutput};
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::{json, Value};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec { format: FormatId::Arrow, params }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}

fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        ("ints-single-column", json!({"columns": [{"name": "n", "type": "int32"}], "rows": [[1], [2], [3]]})),
        (
            "mixed-basic",
            json!({
                "columns": [
                    {"name": "id", "type": "int32"},
                    {"name": "name", "type": "utf8"},
                    {"name": "active", "type": "bool"},
                    {"name": "score", "type": "double"}
                ],
                "rows": [
                    [1, "alpha", true, 0.5],
                    [2, "beta", false, -1.25],
                    [3, null, true, null],
                    [4, "delta", null, 2.0],
                    [5, "epsilon", true, 100.125]
                ]
            }),
        ),
        (
            "nulls-everywhere",
            json!({
                "columns": [
                    {"name": "a", "type": "int32"},
                    {"name": "b", "type": "utf8"},
                    {"name": "c", "type": "double"},
                    {"name": "d", "type": "bool"}
                ],
                "rows": [[null, null, null, null]]
            }),
        ),
        (
            "some-nulls",
            json!({
                "columns": [{"name": "x", "type": "int64"}, {"name": "y", "type": "utf8"}],
                "rows": [[1, "one"], [null, null], [3, null]]
            }),
        ),
        (
            "unicode-strings",
            json!({
                "columns": [{"name": "label", "type": "utf8"}],
                "rows": [["日本語"], ["ελλάδα"], ["✓ plain"]]
            }),
        ),
        (
            "integer-bounds",
            json!({
                "columns": [{"name": "i32c", "type": "int32"}, {"name": "i64c", "type": "int64"}],
                "rows": [[2147483647, 9223372036854775807i64], [-2147483648i64, (-9223372036854775808i64)]]
            }),
        ),
        (
            "doubles-fractional",
            json!({
                "columns": [{"name": "v", "type": "double"}],
                "rows": [[0.1], [-0.3333333333333333], [1e300]]
            }),
        ),
        ("zero-rows", json!({"columns": [{"name": "a", "type": "int32"}], "rows": []})),
        (
            "single-row-all-types",
            json!({
                "columns": [
                    {"name": "i", "type": "int32"},
                    {"name": "I", "type": "int64"},
                    {"name": "d", "type": "double"},
                    {"name": "s", "type": "utf8"},
                    {"name": "b", "type": "bool"}
                ],
                "rows": [[7, 7000000000i64, 7.25, "seven", true]]
            }),
        ),
        (
            "string-extremes",
            json!({
                "columns": [{"name": "s", "type": "utf8"}],
                "rows": [[""], ["a"], ["z"]]
            }),
        ),
        (
            "bools-only",
            json!({
                "columns": [{"name": "flag", "type": "bool"}],
                "rows": [[true], [false], [true]]
            }),
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
        let parsed = TabularSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: spec must parse: {error}"));
        let expected = arrow::expected_tree(&parsed);
        let bytes = arrow::generate(&input)
            .unwrap_or_else(|error| panic!("{name}: generate: {error}"));
        let verdict = arrow::verify(&expected, &bytes)
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
        let parsed = TabularSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let expected = arrow::expected_tree(&parsed);
        let generated = registry
            .dispatch(FormatId::Arrow, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(FormatId::Arrow, OpRequest::Verify { expected: &expected, bytes })
            .unwrap_or_else(|error| panic!("{name}: dispatch verify: {error}"));
        let OpOutput::Verify(verdict) = verified.inner() else {
            panic!("{name}: expected Verify output");
        };
        assert!(verdict.is_pass(), "{name}: executor-path round trip failed");
    }
}

#[test]
fn committed_fixture_extracts_the_pinned_tree() {
    let extracted =
        arrow::extract(&fixture("arrow-doc.arrow"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "format": "arrow",
            "num_rows": 5,
            "schema": [
                {"name": "id", "type": "int32"},
                {"name": "name", "type": "utf8"},
                {"name": "active", "type": "bool"},
                {"name": "score", "type": "double"}
            ],
            "batches": [{"rows": 5, "columns": 4}],
            "rows": [
                [1, "alpha", true, 0.5],
                [2, "beta", false, -1.25],
                [3, null, true, null],
                [4, "delta", null, 2.0],
                [5, "epsilon", true, 100.125]
            ]
        })
    );
    assert!(extracted.text_projection.contains("5 rows"));
}

#[test]
fn truncated_file_is_typed_parse_failure() {
    // The M2 magic-stub fixture: ARROW1 + continuation byte, no messages.
    let stub = crate_fixture_bytes("arrow.arrow");
    let error = arrow::extract(&stub, ExtractBudgets::default())
        .expect_err("stub must not parse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
    let error = arrow::extract(b"ARROW1\xff\x00", ExtractBudgets::default())
        .expect_err("short tail must not parse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn schema_mismatch_folds_into_the_verify_diff() {
    // Generate for one spec, verify against another spec's tree: the
    // re-read + Schema::eq path must surface a typed diff, never pass.
    let input = spec(corpus()[1].1.clone());
    let bytes = arrow::generate(&input).expect("generate");
    let other = TabularSpec::from_generation(&spec(corpus()[9].1.clone())).expect("spec");
    let verdict = arrow::verify(&arrow::expected_tree(&other), &bytes)
        .expect("verify runs");
    assert!(!verdict.is_pass(), "mismatched spec must fail the gate");
}

#[test]
fn generation_is_byte_deterministic() {
    let input = spec(corpus()[1].1.clone());
    assert_eq!(
        arrow::generate(&input).expect("first"),
        arrow::generate(&input).expect("second")
    );
}

#[test]
fn generated_bytes_carry_the_arrow_file_magic() {
    let bytes = arrow::generate(&spec(corpus()[0].1.clone())).expect("generate");
    assert!(bytes.starts_with(b"ARROW1"));
    assert!(bytes.ends_with(b"ARROW1"));
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    let bytes = fixture("arrow-doc.arrow");
    let tiny = ExtractBudgets { max_total_bytes: 4, ..ExtractBudgets::default() };
    let error = arrow::extract(&bytes, tiny).expect_err("tiny cap");
    assert!(matches!(error, MediaError::ExtractTooLarge), "got {error:?}");
}

#[test]
fn invalid_specs_are_typed() {
    let invalid = [
        json!({"columns": [{"name": "a", "type": "date32"}], "rows": []}),
        json!({"columns": [{"name": "a", "type": "int32"}], "rows": [[true]]}),
    ];
    for params in invalid {
        let case = params.to_string();
        let error =
            arrow::generate(&spec(params)).expect_err(&format!("params must be refused: {case}"));
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
}

#[test]
fn projector_registers_for_arrow_through_the_data_lane() {
    let mut registry = harness_media::projection::ProjectionRegistry::new();
    harness_media::adapter::data::register_projections(&mut registry).expect("register");
    let context = harness_media::projection::ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    let projected = registry
        .project(
            &FormatId::Arrow,
            harness_core::event::ProjectionKind::TextProjection,
            &fixture("arrow-doc.arrow"),
            &context,
        )
        .expect("project");
    assert!(projected.contains("5 rows"));
}

fn crate_fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}
