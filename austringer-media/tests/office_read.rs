//! M18 office read adapter tests (pure tier: OOXML extract via zip +
//! quick-xml). Hermetic: committed fixtures, zero network, zero
//! external tools, zero filesystem writes.
//!
//! Acceptance pins (plan M18): hand-committed minimal fixtures extract
//! to pinned trees; a formula cell shows `=SUM(...)` text (never
//! evaluated); a macro-bearing fixture reports "macros present: 1,
//! not executed" (listed, never executed); corrupt packages fail the
//! typed ParseFailure.

use austringer_media::adapter::office;
use austringer_media::error::MediaError;
use austringer_media::executor::{OpOutput, OpRequest};
use austringer_media::extract::ExtractBudgets;
use austringer_media::format::FormatId;
use austringer_media::projection::{ProjectionContext, ProjectionRegistry};
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("fixtures/office/{name}");
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {path}: {error}"))
}

fn tiny_budgets() -> ExtractBudgets {
    ExtractBudgets {
        deadline: std::time::Duration::from_secs(10),
        max_total_bytes: 1,
    }
}

#[test]
fn docx_fixture_extracts_to_pinned_tree() {
    // Given: the hand-committed minimal docx fixture (headings, styles,
    // a table, a bullet list, an embedded image, an external rel).
    // When: extracted.
    let extracted = office::docx::extract(&fixture("docx.docx"), ExtractBudgets::default())
        .expect("docx extract must succeed");
    // Then: the pinned canonical tree.
    assert_eq!(
        extracted.tree,
        json!({
            "external_rels": ["https://example.com/external"],
            "images": ["media/image1.png"],
            "macros": {"present": 0},
            "paragraphs": [
                {"list": false, "style": "Heading1", "text": "Title"},
                {"list": false, "style": null, "text": "Body one."},
                {"list": false, "style": "Quirky", "text": "Styled but undefined."},
                {"list": true, "style": null, "text": "bullet"},
                {"list": true, "style": null, "text": "bullet two"},
            ],
            "styles_defined": ["Heading1", "Heading2", "ListParagraph", "Normal"],
            "tables": [{"cols": 2, "rows": 2}],
        })
    );
}

#[test]
fn docx_projection_is_bounded_markdownish_text() {
    let extracted = office::docx::extract(&fixture("docx.docx"), ExtractBudgets::default())
        .expect("docx extract must succeed");
    assert!(
        extracted.text_projection.contains("# Title"),
        "heading renders markdown-ish: {}",
        extracted.text_projection
    );
    assert!(extracted.text_projection.contains("- bullet"));
    assert!(extracted.text_projection.contains("table: 2 rows x 2 cols"));
    assert!(extracted.text_projection.chars().count() <= office::TEXT_PROJECTION_MAX_CHARS);
}

#[test]
fn xlsx_fixture_extracts_to_pinned_tree() {
    let extracted = office::xlsx::extract(&fixture("xlsx.xlsx"), ExtractBudgets::default())
        .expect("xlsx extract must succeed");
    assert_eq!(
        extracted.tree,
        json!({
            "macros": {"present": 0},
            "sheets": [
                {
                    "dimension": {"cols": 3, "rows": 3},
                    "formulas": ["C1 =SUM(B1:B2)"],
                    "head_cells": [
                        {"kind": "string", "ref": "A1", "value": "Alpha"},
                        {"kind": "number", "ref": "B1", "value": "1"},
                        {"kind": "formula", "ref": "C1", "value": "=SUM(B1:B2)"},
                        {"kind": "string", "ref": "A2", "value": "Beta"},
                        {"kind": "number", "ref": "B2", "value": "2"},
                        {"kind": "string", "ref": "A3", "value": "Gamma"},
                    ],
                    "name": "Data",
                },
                {
                    "dimension": {"cols": 1, "rows": 1},
                    "formulas": [],
                    "head_cells": [{"kind": "number", "ref": "A1", "value": "42"}],
                    "name": "Stats",
                },
            ],
            "styles_defined": ["Comma", "Normal"],
        })
    );
}

#[test]
fn formula_cell_is_listed_as_text_with_equals_prefix() {
    // Given: sheet1 C1 carries <f>SUM(B1:B2)</f> with a cached <v>3</v>.
    let extracted = office::xlsx::extract(&fixture("xlsx.xlsx"), ExtractBudgets::default())
        .expect("xlsx extract must succeed");
    // Then: the formula is listed AS TEXT with the `=` prefix - the
    // cached value is stored data, the formula is never evaluated.
    let sheets = extracted.tree["sheets"].as_array().expect("sheets array");
    let cells = sheets[0]["head_cells"].as_array().expect("cells");
    let formula = cells
        .iter()
        .find(|cell| cell["ref"] == json!("C1"))
        .expect("C1 present");
    assert_eq!(formula["kind"], json!("formula"));
    assert_eq!(formula["value"], json!("=SUM(B1:B2)"));
    assert_eq!(sheets[0]["formulas"], json!(["C1 =SUM(B1:B2)"]));
}

#[test]
fn xlsx_projection_is_per_sheet_schema_and_stats() {
    let extracted = office::xlsx::extract(&fixture("xlsx.xlsx"), ExtractBudgets::default())
        .expect("xlsx extract must succeed");
    assert!(
        extracted.text_projection.starts_with("xlsx: 2 sheets"),
        "projection names every sheet: {}",
        extracted.text_projection
    );
    assert!(extracted.text_projection.contains("Data: 3 rows x 3 cols"));
    assert!(extracted.text_projection.contains("=SUM(B1:B2)"));
}

#[test]
fn pptx_fixture_extracts_to_pinned_tree() {
    let extracted = office::pptx::extract(&fixture("pptx.pptx"), ExtractBudgets::default())
        .expect("pptx extract must succeed");
    assert_eq!(
        extracted.tree,
        json!({
            "macros": {"present": 0},
            "slide_count": 2,
            "slides": [
                {
                    "shapes": {"graphic_frame": 0, "pic": 0, "sp": 2},
                    "text_runs": ["Slide title", "bullet one", "bullet two"],
                },
                {
                    "shapes": {"graphic_frame": 1, "pic": 1, "sp": 0},
                    "text_runs": ["chart frame"],
                },
            ],
        })
    );
}

#[test]
fn pptx_projection_names_slides_shapes_and_texts() {
    let extracted = office::pptx::extract(&fixture("pptx.pptx"), ExtractBudgets::default())
        .expect("pptx extract must succeed");
    assert!(
        extracted.text_projection.starts_with("pptx: 2 slides"),
        "projection: {}",
        extracted.text_projection
    );
    assert!(extracted.text_projection.contains("slide 1: 2 shapes"));
}

#[test]
fn macro_bearing_fixture_discloses_macros_without_executing() {
    // Given: the docx fixture plus a word/vbaProject.bin part.
    let extracted = office::docx::extract(&fixture("docx-macro.docx"), ExtractBudgets::default())
        .expect("docx-macro extract must succeed");
    // Then: macro presence is LISTED (counted, disclosed) and the
    // projection reports it verbatim - never executed, never extracted.
    assert_eq!(extracted.tree["macros"], json!({"present": 1}));
    assert!(
        extracted
            .text_projection
            .contains("macros present: 1, not executed"),
        "disclosure line verbatim: {}",
        extracted.text_projection
    );
}

#[test]
fn applied_style_absent_from_styles_census_stays_visible() {
    // Given: a paragraph styled "Quirky" while styles.xml defines only
    // Normal/Heading1/Heading2/ListParagraph (the M19 round-trip guard:
    // a writer with broken style definitions must fail the diff).
    let extracted = office::docx::extract(&fixture("docx.docx"), ExtractBudgets::default())
        .expect("docx extract must succeed");
    let styles = extracted.tree["styles_defined"].as_array().expect("census");
    assert!(
        !styles.iter().any(|name| name == &json!("Quirky")),
        "census lists only DEFINED styles"
    );
    let paragraphs = extracted.tree["paragraphs"].as_array().expect("paragraphs");
    assert_eq!(paragraphs[2]["style"], json!("Quirky"));
}

#[test]
fn corrupt_zip_fails_typed_parse_failure() {
    // Given: zip magic followed by garbage (no central directory).
    let corrupt = b"PK\x03\x04 definitely not a zip".as_slice();
    for result in [
        office::docx::extract(corrupt, ExtractBudgets::default()),
        office::xlsx::extract(corrupt, ExtractBudgets::default()),
        office::pptx::extract(corrupt, ExtractBudgets::default()),
    ] {
        // Then: the typed ParseFailure - never a panic, never a guess.
        assert!(
            matches!(result, Err(MediaError::ParseFailure)),
            "corrupt zip must be ParseFailure, got {result:?}"
        );
    }
}

#[test]
fn missing_document_part_is_parse_failure() {
    // Given: a package whose content types claim wordprocessingml but
    // which carries no word/document.xml part.
    let error = office::docx::extract(&fixture("docx-no-document.docx"), ExtractBudgets::default())
        .expect_err("missing document part must refuse");
    assert!(matches!(error, MediaError::ParseFailure));
}

#[test]
fn xlsx_bytes_through_the_docx_extractor_refuse_typed() {
    let error = office::docx::extract(&fixture("xlsx.xlsx"), ExtractBudgets::default())
        .expect_err("xlsx bytes are not a docx package");
    assert!(matches!(error, MediaError::ParseFailure));
}

#[test]
fn budget_trips_are_typed_extract_too_large() {
    for result in [
        office::docx::extract(&fixture("docx.docx"), tiny_budgets()),
        office::xlsx::extract(&fixture("xlsx.xlsx"), tiny_budgets()),
        office::pptx::extract(&fixture("pptx.pptx"), tiny_budgets()),
    ] {
        assert!(
            matches!(result, Err(MediaError::ExtractTooLarge)),
            "size-cap trip must be ExtractTooLarge, got {result:?}"
        );
    }
}

#[test]
fn executor_dispatch_runs_office_extract_with_pure_disclosure() {
    // Given: the day-one executor registry (all external tools off).
    let registry = austringer_media::executor::registry();
    for (format, name) in [
        (FormatId::Docx, "docx.docx"),
        (FormatId::Xlsx, "xlsx.xlsx"),
        (FormatId::Pptx, "pptx.pptx"),
    ] {
        // When: extract is dispatched.
        let produced = registry
            .dispatch(
                format,
                OpRequest::Extract {
                    bytes: &fixture(name),
                    budgets: ExtractBudgets::default(),
                },
            )
            .unwrap_or_else(|error| panic!("{format} extract dispatch: {error}"));
        // Then: an Extract artifact whose producer discloses the pure
        // office engine (pptx extract is pure even though pptx
        // GENERATION rides the pandoc subprocess lane).
        assert!(
            matches!(produced.inner(), OpOutput::Extract(_)),
            "{format}: extract output"
        );
        let producer = produced.producer();
        assert_eq!(producer.tier, austringer_media::executor::AdapterTier::Pure);
        assert_eq!(producer.engine, "austringer-media::office");
        assert_eq!(producer.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(producer.command, None);
    }
}

#[test]
fn generate_and_verify_refusals_cite_the_m19_write_lane() {
    // Given: M18 wires extract only - the office writers are M19.
    let registry = austringer_media::executor::registry();
    let spec = austringer_media::generate::GenerationSpec {
        format: FormatId::Xlsx,
        params: json!({}),
    };
    let error = registry
        .dispatch(FormatId::Xlsx, OpRequest::Generate { spec: &spec })
        .expect_err("xlsx generate must refuse until M19");
    match error {
        MediaError::UnsupportedOperation { format, op, reason } => {
            assert_eq!(format, FormatId::Xlsx);
            assert_eq!(op, austringer_media::descriptor::Operation::Generate);
            assert!(reason.contains("M19"), "reason cites the lane: {reason}");
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
    let error = registry
        .dispatch(
            FormatId::Docx,
            OpRequest::Verify {
                expected: &json!({}),
                bytes: b"",
            },
        )
        .expect_err("docx verify must refuse until M19");
    match error {
        MediaError::UnsupportedOperation { reason, .. } => {
            assert!(reason.contains("M19"), "reason cites the lane: {reason}");
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn office_projectors_wrap_the_extract_projection() {
    // Given: the deterministic projector registration (M10 seam).
    let mut projections = ProjectionRegistry::new();
    office::register_projections(&mut projections).expect("no duplicate projectors");
    let context = ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    for (format, name) in [
        (FormatId::Docx, "docx.docx"),
        (FormatId::Xlsx, "xlsx.xlsx"),
        (FormatId::Pptx, "pptx.pptx"),
    ] {
        // When: projected through the registry.
        let projected = projections
            .project(
                &format,
                austringer_core_projection_kind(),
                &fixture(name),
                &context,
            )
            .unwrap_or_else(|error| panic!("{format} projection: {error}"));
        // Then: identical to the extractor's own text projection - a
        // thin wrap, never a fabricated transcript.
        let extracted = match format {
            FormatId::Docx => office::docx::extract(&fixture(name), ExtractBudgets::default()),
            FormatId::Xlsx => office::xlsx::extract(&fixture(name), ExtractBudgets::default()),
            _ => office::pptx::extract(&fixture(name), ExtractBudgets::default()),
        }
        .expect("extract");
        assert_eq!(projected, extracted.text_projection);
    }
}

fn austringer_core_projection_kind() -> austringer_core::event::ProjectionKind {
    austringer_core::event::ProjectionKind::TextProjection
}
