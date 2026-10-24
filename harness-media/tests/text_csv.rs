//! M30 csv lane: RFC 4180 via the csv crate - schema + row-count
//! extract, deterministic writer (CRLF, no BOM), dialect/BOM hazards
//! handled and disclosed, never guessed. Hermetic.

use std::sync::Arc;

use harness_media::adapter::text::csv::{self, CsvSpec};
use harness_media::error::MediaError;
use harness_media::executor::{OpOutput, OpRequest};
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use harness_media::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};
use serde_json::{Value, json};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Csv,
        params,
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name} ({path}): {error}"))
}

fn rows(rows: Value) -> Value {
    json!({"rows": rows})
}

fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        ("single-row", rows(json!([["a", "b", "c"]]))),
        ("header-plus-data", rows(json!([["h1", "h2"], ["1", "2"]]))),
        ("quoted-comma", rows(json!([["before", "a,b", "after"]]))),
        ("quoted-quote", rows(json!([["say \"hi\""]]))),
        ("quoted-newline", rows(json!([["line1\nline2", "x"]]))),
        ("unicode", rows(json!([["日本語", "café"]]))),
        ("empty-field", rows(json!([["", "x", ""]]))),
        (
            "wide-table",
            rows(json!([
                ["c1", "c2", "c3", "c4", "c5"],
                ["1", "2", "3", "4", "5"]
            ])),
        ),
        ("single-column", rows(json!([["only"], ["1"], ["2"]]))),
        (
            "mixed-hazards",
            rows(json!([["h,1", "h\"2"], ["a\nb", "  spaced  "]])),
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
        let expected = CsvSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: spec must parse: {error}"))
            .canonical_tree();
        let bytes =
            csv::generate(&input).unwrap_or_else(|error| panic!("{name}: generate: {error}"));
        let verdict = csv::verify(&expected, &bytes)
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
        let expected = CsvSpec::from_generation(&input)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .canonical_tree();
        let generated = registry
            .dispatch(FormatId::Csv, OpRequest::Generate { spec: &input })
            .unwrap_or_else(|error| panic!("{name}: dispatch generate: {error}"));
        let OpOutput::Generate(bytes) = generated.inner() else {
            panic!("{name}: expected Generate output");
        };
        let verified = registry
            .dispatch(
                FormatId::Csv,
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
fn minimal_fixture_extracts_the_pinned_schema() {
    let extracted = csv::extract(&fixture("csv.csv"), ExtractBudgets::default()).expect("extract");
    assert_eq!(
        extracted.tree,
        json!({
            "bom": false,
            "field_count": 3,
            "first_row": ["a", "b", "c"],
            "row_count": 3,
        })
    );
    assert!(extracted.text_projection.contains("3 rows"));
    assert!(extracted.text_projection.contains("3 fields"));
}

#[test]
fn bomd_fixture_extracts_with_the_dialect_disclosed() {
    // BOM read-tolerant: stripped before parse, disclosed in the tree.
    let extracted =
        csv::extract(&fixture("csv-bom.csv"), ExtractBudgets::default()).expect("extract");
    assert_eq!(extracted.tree["bom"], json!(true));
    assert_eq!(extracted.tree["row_count"], json!(2));
    assert_eq!(extracted.tree["first_row"], json!(["x", "y"]));
}

#[test]
fn generated_output_is_crlf_and_bom_free() {
    // RFC 4180 records are CRLF-delimited; the writer NEVER emits a BOM
    // (write-BOM disclosed: we do not write one).
    let input = spec(rows(json!([["a", "b"], ["1", "2"]])));
    let bytes = csv::generate(&input).expect("generate");
    assert!(bytes.ends_with(b"\r\n"), "CRLF-terminated: {bytes:?}");
    assert!(
        bytes.windows(2).any(|w| w == b"\r\n"),
        "CRLF between records"
    );
    assert_ne!(&bytes[..3], b"\xef\xbb\xbf", "no BOM written");
}

#[test]
fn ragged_input_is_typed_parse_failure() {
    // Dialect hazard never guessed: inconsistent field counts refuse
    // (flexible=false), they are not silently padded.
    let error =
        csv::extract(b"a,b\n1\n", ExtractBudgets::default()).expect_err("ragged rows must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn empty_input_extracts_an_empty_table() {
    let extracted = csv::extract(b"", ExtractBudgets::default()).expect("empty csv is valid");
    assert_eq!(
        extracted.tree,
        json!({"bom": false, "field_count": 0, "first_row": null, "row_count": 0})
    );
}

#[test]
fn non_utf8_is_typed_parse_failure() {
    let error = csv::extract(&[0xff, 0x2c], ExtractBudgets::default()).expect_err("must refuse");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    let bytes = fixture("csv.csv");
    let zero = ExtractBudgets {
        deadline: std::time::Duration::ZERO,
        ..ExtractBudgets::default()
    };
    let error = csv::extract(&bytes, zero).expect_err("zero deadline");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
    let tiny = ExtractBudgets {
        max_total_bytes: 4,
        ..ExtractBudgets::default()
    };
    let error = csv::extract(&bytes, tiny).expect_err("tiny cap");
    assert!(
        matches!(error, MediaError::ExtractTooLarge),
        "got {error:?}"
    );
}

#[test]
fn generation_is_byte_deterministic() {
    let input = spec(corpus()[4].1.clone());
    assert_eq!(
        csv::generate(&input).expect("1"),
        csv::generate(&input).expect("2")
    );
}

#[test]
fn invalid_specs_are_typed() {
    let invalid = [
        rows(json!([["a", "b"], ["only-one"]])),
        rows(json!([])),
        json!({"value": [["a"]]}),
    ];
    for params in invalid {
        let error = csv::generate(&spec(params)).expect_err("must be InvalidSpec");
        assert!(matches!(error, MediaError::InvalidSpec), "got {error:?}");
    }
    let foreign = GenerationSpec {
        format: FormatId::Json,
        params: json!({"rows": [["a"]]}),
    };
    assert!(matches!(
        csv::generate(&foreign),
        Err(MediaError::InvalidSpec)
    ));
}

#[test]
fn projector_is_a_thin_wrap_over_the_extractor() {
    let bytes = fixture("csv-bom.csv");
    let extracted = csv::extract(&bytes, ExtractBudgets::default()).expect("extract");
    let context = ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    let projected = csv::CsvProjector
        .project(&bytes, &context)
        .expect("project");
    assert_eq!(projected, extracted.text_projection);

    let mut registry = ProjectionRegistry::new();
    registry
        .register(FormatId::Csv, Arc::new(csv::CsvProjector))
        .expect("register");
    let mismatch = registry
        .project(
            &FormatId::Csv,
            harness_core::event::ProjectionKind::Transcript,
            &bytes,
            &context,
        )
        .expect_err("kind mismatch is typed");
    assert!(
        matches!(mismatch, MediaError::NoProjector),
        "got {mismatch:?}"
    );
}
