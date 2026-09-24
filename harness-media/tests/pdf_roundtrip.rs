//! M13 pdf adapter generate/verify/dispatch tests: spec->bytes (krilla
//! simple-layout writer), the round-trip gate on a text corpus INCLUDING
//! non-Latin1 entries (Cyrillic + CJK) with fonts-embedded asserted via
//! the extract font list, byte determinism under the fixed-epoch contract,
//! typed InvalidSpec refusals, executor wiring with producer disclosure,
//! and the deterministic TextProjection provider. Hermetic.

use std::sync::Arc;

use harness_core::event::ProjectionKind;
use harness_media::adapter::pdf;
use harness_media::error::MediaError;
use harness_media::executor::{AdapterTier, OpRequest};
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use harness_media::projection::{ProjectionContext, ProjectionRegistry};
use serde_json::{json, Value};

fn spec(params: Value) -> GenerationSpec {
    GenerationSpec { format: FormatId::Pdf, params }
}

/// (name, params) corpus: Latin, Cyrillic, CJK, and a mixed two-page doc.
/// The non-Latin1 entries are the plan's pinned fidelity law: fonts must
/// be embedded with ToUnicode maps - never WinAnsi-for-non-Latin1.
fn corpus() -> Vec<(&'static str, Value)> {
    vec![
        (
            "latin",
            json!({"pages": [{"blocks": [
                {"text": "Hello, harness PDF adapter!", "size": 14.0},
                {"text": "The quick brown fox jumps over the lazy dog.", "size": 11.0},
            ]}]}),
        ),
        (
            "cyrillic",
            json!({"pages": [{"blocks": [
                {"text": "Привет, мир! Съешьте ещё этих мягких французских булок.", "size": 12.0},
            ]}]}),
        ),
        (
            "cjk",
            json!({"pages": [{"blocks": [
                {"text": "你好，世界！这是中文测试。日本語テスト。", "size": 12.0, "font": "cjk"},
            ]}]}),
        ),
        (
            "mixed-two-page",
            json!({"pages": [
                {"blocks": [{"text": "Page one: latin text.", "size": 12.0}]},
                {"blocks": [{"text": "你好，世界！", "size": 12.0, "font": "cjk"}]},
            ]}),
        ),
    ]
}

/// The expected per-page text of a spec, computed WITHOUT the extractor
/// (independent projection inputs; the projection itself lives in pdf.rs).
fn expected_page_texts(params: &Value) -> Vec<String> {
    params["pages"]
        .as_array()
        .expect("pages array")
        .iter()
        .map(|page| {
            page["blocks"]
                .as_array()
                .expect("blocks array")
                .iter()
                .map(|block| block["text"].as_str().expect("block text"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

#[test]
fn roundtrip_corpus_is_text_equal_with_fonts_embedded() {
    for (name, params) in corpus() {
        // Given: a spec from the corpus.
        let generation = spec(params.clone());
        // When: generated then extracted.
        let bytes = pdf::generate(&generation).unwrap_or_else(|e| panic!("{name}: generate {e:?}"));
        let extracted =
            pdf::extract(&bytes, ExtractBudgets::default()).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        // Then: page count and whitespace-normalized text round-trip exactly.
        let expected_texts = expected_page_texts(&params);
        let pages = extracted.tree["pages"].as_array().expect("pages");
        assert_eq!(pages.len(), expected_texts.len(), "{name}: page count");
        for (index, page) in pages.iter().enumerate() {
            let actual = pdf::normalize_ws(page["text"].as_str().unwrap_or_default());
            let expected = pdf::normalize_ws(&expected_texts[index]);
            assert_eq!(actual, expected, "{name}: page {index} text diff");
        }
        // And: every font on every page is EMBEDDED (the #1 agent-PDF
        // defect is unembedded fonts - pinned here via the extract font
        // list, per plan M13).
        for font in extracted.tree["fonts"].as_array().expect("fonts") {
            assert_eq!(font["embedded"], json!(true), "{name}: font {font} must be embedded");
        }
        // And: non-Latin1 entries never ride WinAnsi - krilla emits
        // Type0/Identity-H with ToUnicode CMaps (fidelity law).
        let non_latin1 = name == "cyrillic" || name == "cjk" || name == "mixed-two-page";
        if non_latin1 {
            let fonts = extracted.tree["fonts"].as_array().expect("fonts");
            assert!(
                fonts.iter().any(|font| font["subtype"] == json!("Type0")
                    && font["encoding"] == json!("Identity-H")),
                "{name}: Type0/Identity-H font expected: {:?}",
                serde_json::to_string(fonts).unwrap_or_default()
            );
            for font in fonts {
                assert_ne!(
                    font["encoding"], json!("WinAnsiEncoding"),
                    "{name}: WinAnsi emitted for non-Latin1 text - the pinned defect"
                );
            }
        }
    }
}

#[test]
fn verify_gate_passes_on_corpus_and_catches_a_mutated_expectation() {
    for (name, params) in corpus() {
        // Given: generated bytes and the INDEPENDENT spec->tree projection.
        let generation = spec(params.clone());
        let bytes = pdf::generate(&generation).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let expected = pdf::spec_tree(&generation).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        // When: verified (text diff with whitespace normalization).
        let verdict = pdf::verify(&expected, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        // Then: PASS - the round-trip gate is empty-diff.
        assert!(verdict.is_pass(), "{name}: verdict {verdict:?}");
    }
    // And: a mutated expectation FAILS (the gate has teeth).
    let params = corpus()[0].1.clone();
    let generation = spec(params);
    let bytes = pdf::generate(&generation).expect("generate");
    let mut mutated = pdf::spec_tree(&generation).expect("spec tree");
    mutated["pages"][0]["text"] = json!("not the produced text");
    let verdict = pdf::verify(&mutated, &bytes).expect("verify");
    assert!(!verdict.is_pass(), "mutated expectation must not pass");
}

#[test]
fn generation_is_byte_deterministic_with_epoch_dates() {
    // Given: one corpus spec, generated twice.
    let generation = spec(corpus()[1].1.clone());
    let first = pdf::generate(&generation).expect("generate 1");
    let second = pdf::generate(&generation).expect("generate 2");
    // Then: byte-identical output (M1 determinism contract).
    assert_eq!(first, second, "repeat generation must be byte-identical");
    // And: the document clock is pinned to the fixed epoch (2026-01-01).
    let extracted = pdf::extract(&first, ExtractBudgets::default()).expect("extract");
    let date = extracted.tree["metadata"]["creation_date"].as_str().expect("date");
    assert!(date.starts_with("D:20260101"), "creation date pinned to epoch, got {date}");
}

#[test]
fn invalid_specs_fail_typed_invalid_spec() {
    // Given: specs that violate the adapter contract.
    let bad: Vec<Value> = vec![
        json!({}),
        json!({"pages": "not an array"}),
        json!({"pages": []}),
        json!({"pages": [{"blocks": [{"text": 42, "size": 12.0}]}]}),
        json!({"pages": [{"blocks": [{"text": "x", "size": 0.5}]}]}),
        json!({"pages": [{"blocks": [{"text": "x", "size": 12.0, "font": "nope"}]}]}),
        json!({"pages": [{"blocks": []}], "page_size": {"width": 5.0, "height": 100.0}}),
    ];
    for params in bad {
        // When: generated.
        let error = pdf::generate(&spec(params)).err();
        // Then: typed InvalidSpec - never a panic, never fake output.
        assert!(matches!(error, Some(MediaError::InvalidSpec)), "got {error:?}");
    }
}

#[test]
fn executor_dispatch_wires_pdf_ops_with_producer_disclosure() {
    // Given: the day-one executor registry with the pdf feature on.
    let registry = harness_media::executor::registry();
    // When: generate is dispatched for a valid spec.
    let generation = spec(corpus()[0].1.clone());
    let produced = registry
        .dispatch(FormatId::Pdf, OpRequest::Generate { spec: &generation })
        .expect("pdf generate dispatches");
    // Then: the artifact carries producer disclosure; the writer engine
    // states its SIMPLE TEXT LAYOUT honestly (plan M13).
    assert_eq!(produced.producer().tier, AdapterTier::Pure);
    assert_eq!(produced.producer().engine, pdf::GENERATE_ENGINE);
    assert!(produced.producer().engine.contains("simple-layout"));

    // And: extract + verify dispatch through the same registry.
    let bytes = fixture_bytes_from(produced.inner());
    let extracted = registry
        .dispatch(
            FormatId::Pdf,
            OpRequest::Extract { bytes: &bytes, budgets: ExtractBudgets::default() },
        )
        .expect("pdf extract dispatches");
    assert_eq!(extracted.producer().engine, pdf::EXTRACT_ENGINE);
    let expected = pdf::spec_tree(&generation).expect("spec tree");
    let verified = registry
        .dispatch(
            FormatId::Pdf,
            OpRequest::Verify { expected: &expected, bytes: &bytes },
        )
        .expect("pdf verify dispatches");
    assert!(
        matches!(verified.inner(), harness_media::executor::OpOutput::Verify(v) if v.is_pass()),
        "dispatched verify passes"
    );

    // And: the M2 descriptor table claims the three landed ops.
    let registry_table = harness_media::probe::registry();
    let descriptor = registry_table.lookup(&FormatId::Pdf).expect("descriptor");
    for op in [
        harness_media::descriptor::Operation::Extract,
        harness_media::descriptor::Operation::Generate,
        harness_media::descriptor::Operation::Verify,
    ] {
        assert!(
            matches!(descriptor.ops.get(&op), Some(harness_media::descriptor::OpSupport::Supported)),
            "pdf {op} must be claimed Supported"
        );
    }
}

fn fixture_bytes_from(output: &harness_media::executor::OpOutput) -> Vec<u8> {
    match output {
        harness_media::executor::OpOutput::Generate(bytes) => bytes.clone(),
        other => panic!("expected generate output, got {other:?}"),
    }
}

#[test]
fn pdf_projection_provider_is_a_thin_extract_wrap() {
    // Given: the adapter's deterministic TextProjection provider, registered.
    let mut projections = ProjectionRegistry::new();
    projections
        .register(FormatId::Pdf, Arc::new(pdf::PdfProjection))
        .expect("pdf projector registers");
    let bytes = {
        let path = format!("{}/fixtures/pdf/minimal.pdf", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).expect("minimal fixture")
    };
    let context = ProjectionContext { budgets: ExtractBudgets::default(), name_hint: Some("a.pdf") };
    // When: projected.
    let projected = projections
        .project(&FormatId::Pdf, ProjectionKind::TextProjection, &bytes, &context)
        .expect("projection exists");
    // Then: it is the extract-side bounded text projection (M10 seam: thin
    // wrap over Extracted.text_projection).
    assert!(projected.contains("Hello, PDF!"));
    // And: kind mismatches answer the typed NoProjector (never a guess).
    let wrong = projections.project(
        &FormatId::Pdf,
        ProjectionKind::Transcript,
        &bytes,
        &ProjectionContext { budgets: ExtractBudgets::default(), name_hint: None },
    );
    assert!(matches!(wrong, Err(MediaError::NoProjector)));
}
