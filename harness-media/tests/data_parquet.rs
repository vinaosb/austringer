//! M22 parquet lane: arrow-rs `parquet` (pure tier, full round-trip).
//! Extract = schema + row-group/column statistics census + bounded head
//! sample; generate = single-row-group spec-driven write (ordered-exact,
//! statistics on); verify = M5 structural diff against the INDEPENDENT
//! spec->tree projection. Hermetic.

use harness_media::adapter::data::TabularSpec;
use harness_media::adapter::data::parquet;
use harness_media::error::MediaError;
use harness_media::executor::{OpOutput, OpRequest};
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::{Value, json};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Parquet,
        params,
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}

fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        (
            "ints-single-column",
            json!({"columns": [{"name": "n", "type": "int32"}], "rows": [[1], [2], [3]]}),
        ),
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
                "rows": [[null, null, null, null], [null, null, null, null]]
            }),
        ),
        (
            "some-nulls",
            json!({
                "columns": [{"name": "x", "type": "int64"}, {"name": "y", "type": "utf8"}],
                "rows": [[1, "one"], [null, null], [3, null], [null, "four"]]
            }),
        ),
        (
            "unicode-strings",
            json!({
                "columns": [{"name": "label", "type": "utf8"}],
                "rows": [["日本語"], ["ελλάδα"], ["✓ plain"], ["żółć"]]
            }),
        ),
        (
            "integer-bounds",
            json!({
                "columns": [{"name": "i32c", "type": "int32"}, {"name": "i64c", "type": "int64"}],
                "rows": [
                    [2147483647, 9223372036854775807i64],
                    [-2147483648i64, (-9223372036854775808i64)],
                    [0, 0]
                ]
            }),
        ),
        (
            "doubles-fractional",
            json!({
                "columns": [{"name": "v", "type": "double"}],
                "rows": [[0.1], [-0.3333333333333333], [1e300], [0.0], [-1.5e-10]]
            }),
        ),
        (
            "zero-rows",
            json!({"columns": [{"name": "a", "type": "int32"}], "rows": []}),
        ),
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
                "rows": [[""], ["a"], ["z"], ["zz"]]
            }),
        ),
        (
            "bools-only",
            json!({
                "columns": [{"name": "flag", "type": "bool"}],
                "rows": [[true], [false], [true], [true], [false], [false]]
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
        let expected = parquet::expected_tree(&parsed);
        let bytes =
            parquet::generate(&input).unwrap_or_else(|error| panic!("{name}: generate: {error}"));
        let verdict = parquet::verify(&expected, &bytes)
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
        let parsed =
            TabularSpec::from_generation(&input).unwrap_or_else(|error| panic!("{name}: {error}"));
        let expected = parquet::expected_tree(&parsed);
        let generated = registry
            .dispatch(FormatId::Parquet, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(
                FormatId::Parquet,
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
fn committed_fixture_extracts_the_pinned_tree() {
    let extracted = parquet::extract(&fixture("parquet-doc.parquet"), ExtractBudgets::default())
        .expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "format": "parquet",
            "num_rows": 5,
            "row_groups": [{
                "num_rows": 5,
                "columns": [
                    {"name": "id", "stats": {"min": 1, "max": 5, "nulls": 0}},
                    {"name": "name", "stats": {"min": "alpha", "max": "epsilon", "nulls": 1}},
                    {"name": "active", "stats": {"min": false, "max": true, "nulls": 1}},
                    {"name": "score", "stats": {"min": -1.25, "max": 100.125, "nulls": 1}}
                ]
            }],
            "schema": [
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
        })
    );
    let projection = &extracted.text_projection;
    assert!(
        projection.contains("parquet"),
        "projection names the format: {projection}"
    );
    assert!(
        projection.contains("5 rows"),
        "projection carries the row count: {projection}"
    );
}

#[test]
fn c_codec_compressed_fixture_is_typed_unsupported_feature() {
    let error = parquet::extract(&fixture("parquet-zstd.parquet"), ExtractBudgets::default())
        .expect_err("zstd-compressed parquet must refuse typed");
    match &error {
        MediaError::UnsupportedFeature {
            format, feature, ..
        } => {
            assert_eq!(*format, FormatId::Parquet);
            assert!(
                feature.contains("ZSTD"),
                "feature names the codec: {feature}"
            );
        }
        other => panic!("expected UnsupportedFeature, got {other:?}"),
    }
}

#[test]
fn truncated_footer_is_typed_parse_failure() {
    // The M2 magic-stub fixture: PAR1 head, PAR1 foot, no metadata between.
    let stub = crate_fixture_bytes("parquet.parquet");
    let error =
        parquet::extract(&stub, ExtractBudgets::default()).expect_err("stub must not parse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
    let short = b"PAR1\x00\x00\x00\x00PAR";
    let error = parquet::extract(short, ExtractBudgets::default())
        .expect_err("truncated tail must not parse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn generation_is_byte_deterministic() {
    let input = spec(corpus()[1].1.clone());
    assert_eq!(
        parquet::generate(&input).expect("first"),
        parquet::generate(&input).expect("second")
    );
}

#[test]
fn generated_bytes_carry_the_parquet_magic() {
    let bytes = parquet::generate(&spec(corpus()[0].1.clone())).expect("generate");
    assert!(bytes.starts_with(b"PAR1"));
    assert!(bytes.ends_with(b"PAR1"));
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    let bytes = fixture("parquet-doc.parquet");
    let tiny = ExtractBudgets {
        max_total_bytes: 4,
        ..ExtractBudgets::default()
    };
    let error = parquet::extract(&bytes, tiny).expect_err("tiny cap");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn invalid_specs_are_typed() {
    let invalid = [
        json!({}),
        json!({"columns": []}),
        json!({"columns": [{"name": "a", "type": "int32"}, {"name": "a", "type": "int32"}], "rows": []}),
        json!({"columns": [{"name": "a", "type": "vector"}], "rows": []}),
        json!({"columns": [{"name": "a", "type": "int32"}], "rows": [["str"]]}),
        json!({"columns": [{"name": "a", "type": "int32"}], "rows": [[1, 2]]}),
        json!({"columns": [{"name": "a", "type": "int32"}], "rows": [[2147483648i64]]}),
        json!({"columns": [{"name": "a", "type": "utf8"}], "rows": [[3]]}),
        json!({"columns": [{"name": "a", "type": "double"}], "rows": [["x"]]}),
        json!({"columns": [{"name": "", "type": "int32"}], "rows": []}),
    ];
    for params in invalid {
        let case = params.to_string();
        let error =
            parquet::generate(&spec(params)).expect_err(&format!("params must be refused: {case}"));
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
    let foreign = GenerationSpec {
        format: FormatId::Arrow,
        params: json!({"columns": [], "rows": []}),
    };
    assert!(matches!(
        parquet::generate(&foreign),
        Err(MediaError::InvalidSpec)
    ));
}

#[test]
fn projection_summarizes_schema_and_stats() {
    let extracted = parquet::extract(&fixture("parquet-doc.parquet"), ExtractBudgets::default())
        .expect("extract");
    let projection = extracted.text_projection;
    assert!(
        projection.contains("id:int32"),
        "schema in projection: {projection}"
    );
    assert!(
        projection.contains("min 1"),
        "stats in projection: {projection}"
    );
    assert!(projection.chars().count() <= 2000, "bounded projection");
}

#[test]
fn projector_registers_for_parquet_through_the_data_lane() {
    let mut registry = harness_media::projection::ProjectionRegistry::new();
    harness_media::adapter::data::register_projections(&mut registry).expect("register");
    let context = harness_media::projection::ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    let projected = registry
        .project(
            &FormatId::Parquet,
            harness_core::event::ProjectionKind::TextProjection,
            &fixture("parquet-doc.parquet"),
            &context,
        )
        .expect("project");
    assert!(projected.contains("5 rows"));
}

fn crate_fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}
