//! M13 pdf adapter extract-side tests: committed fixtures (hand-written
//! simple-font PDF, encrypted refusal, corrupt xref refusal, the M2 magic
//! stub), budget enforcement (size cap + deadline -> typed
//! ExtractTooLarge), and the bounded text projection. Hermetic.

use std::time::Duration;

use harness_media::adapter::pdf;
use harness_media::error::MediaError;
use harness_media::extract::{ExtractBudgets, TEXT_PROJECTION_MAX_CHARS};
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(path).unwrap_or_else(|error| panic!("fixture {name}: {error}"))
}

fn many_page_pdf(pages: usize) -> Vec<u8> {
    let blocks: Vec<_> = (0..4)
        .map(|i| json!({"text": format!("page text block {i}: the quick brown fox"), "size": 10.0}))
        .collect();
    let pages: Vec<_> = (0..pages).map(|_| json!({"blocks": blocks.clone()})).collect();
    let spec = GenerationSpec { format: FormatId::Pdf, params: json!({"pages": pages}) };
    pdf::generate(&spec).expect("many-page pdf generates")
}

#[test]
fn minimal_fixture_extracts_text_pages_version_and_fonts() {
    // Given: the hand-committed simple-font PDF fixture.
    let bytes = fixture("pdf/minimal.pdf");
    // When: extracted under default budgets.
    let extracted = pdf::extract(&bytes, ExtractBudgets::default()).expect("minimal.pdf extracts");
    // Then: the canonical tree carries version, page count, the text layer,
    // and an honest font list (Helvetica standard-14: NOT embedded).
    assert_eq!(extracted.tree["version"], json!("1.4"));
    assert_eq!(extracted.tree["page_count"], json!(1));
    let text = extracted.tree["pages"][0]["text"].as_str().expect("text");
    assert!(
        pdf::normalize_ws(text).contains("Hello, PDF!"),
        "text layer carries the fixture text, got: {text:?}"
    );
    let fonts = extracted.tree["fonts"].as_array().expect("fonts");
    assert_eq!(fonts.len(), 1, "one font: {fonts:?}");
    assert_eq!(fonts[0]["base_font"], json!("Helvetica"));
    assert_eq!(fonts[0]["embedded"], json!(false));
    assert_eq!(extracted.tree["embedded_images"], json!(0));
    // And: the text projection is bounded and carries structure + text.
    assert!(extracted.text_projection.chars().count() <= TEXT_PROJECTION_MAX_CHARS);
    assert!(extracted.text_projection.contains("Hello, PDF!"));
}

#[test]
fn encrypted_fixture_is_refused_typed_encrypted_unsupported() {
    // Given: a structurally valid PDF whose trailer carries /Encrypt.
    let bytes = fixture("pdf/encrypted.pdf");
    // When: extracted.
    let error = pdf::extract(&bytes, ExtractBudgets::default())
        .expect_err("encrypted input must be refused");
    // Then: the typed refusal - never a decrypt attempt, never a panic.
    assert_eq!(error, MediaError::EncryptedUnsupported);
}

#[test]
fn corrupt_xref_and_magic_stub_fail_typed_parse_failure() {
    // Given: a PDF with a destroyed xref table, and the 70-byte M2 magic
    // stub (dangling /Pages reference).
    for name in ["pdf/corrupt.pdf", "pdf.pdf"] {
        // When: extracted.
        let error = pdf::extract(&fixture(name), ExtractBudgets::default()).err();
        // Then: typed ParseFailure for both (the shared enum carries no
        // page-hint payload; recorded in the M13 evidence).
        assert!(
            matches!(error, Some(MediaError::ParseFailure)),
            "{name}: expected ParseFailure, got {error:?}"
        );
    }
}

#[test]
fn many_page_pdf_trips_the_size_cap_typed() {
    // Given: a generated 400-page PDF (the pathological-input lane).
    let bytes = many_page_pdf(400);
    // When: extracted under a 50 KB materialization cap.
    let budgets = ExtractBudgets { deadline: Duration::from_secs(10), max_total_bytes: 50_000 };
    let error = pdf::extract(&bytes, budgets).expect_err("size cap must trip typed");
    // Then: ExtractTooLarge - never a hang, never silent truncation.
    assert_eq!(error, MediaError::ExtractTooLarge);
    // And: the same bytes under default budgets extract fine (page count
    // honored, deadline not tripped by a healthy input).
    let extracted = pdf::extract(&bytes, ExtractBudgets::default()).expect("default budgets ok");
    assert_eq!(extracted.tree["page_count"], json!(400));
}

#[test]
fn zero_deadline_trips_typed_before_any_page_work() {
    // Given: the minimal fixture and an already-elapsed deadline.
    let bytes = fixture("pdf/minimal.pdf");
    let budgets = ExtractBudgets { deadline: Duration::ZERO, max_total_bytes: u64::MAX };
    // When: extracted.
    let error = pdf::extract(&bytes, budgets).expect_err("deadline must trip typed");
    // Then: ExtractTooLarge (the deadline is one of the two budget axes).
    assert_eq!(error, MediaError::ExtractTooLarge);
}
