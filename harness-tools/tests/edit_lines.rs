//! Stage-C tests: edit_lines — anchored application, uniqueness gate,
//! AST-validated syntax gate, CRLF-preserving writes, bounded results.

use harness_tools::edit::{LineEdit, apply_edits, apply_edits_path};
use harness_tools::{LineTable, ToolsError};
use std::path::Path;

fn rust_source() -> String {
    [
        "fn alpha() {",
        "    let value = 1;",
        "    println!(\"{value}\");",
        "}",
        "",
        "fn beta() {",
        "    let other = 2;",
        "    other + 1",
        "}",
    ]
    .join("\n")
        + "\n"
}

fn single(line: usize, old: &str, new: &str) -> LineEdit {
    LineEdit {
        line,
        old_text: old.to_owned(),
        new_text: new.to_owned(),
        hash: None,
    }
}

#[test]
fn happy_edit_returns_changed_lines_and_fringe_not_file() {
    let table = LineTable::from_string(rust_source());
    let outcome = apply_edits(
        Path::new("sample.rs"),
        &table,
        &[single(7, "    let other = 2;", "    let other = 20;")],
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(outcome.changed_lines, vec![7]);
    // ±3 fringe: lines 4..=10 minus nothing (file is 10 lines).
    assert_eq!(
        (outcome.fringe.first_line, outcome.fringe.last_line),
        (4, 9)
    );
    assert!(outcome.fringe.render().contains("let other = 20;"));
    // Line-exact content assertion (misleading-success guard).
    assert_eq!(outcome.table.content_opt(7), Some("    let other = 20;"));
    assert_eq!(outcome.table.content_opt(6), Some("fn beta() {"));
    assert_eq!(outcome.table.line_count(), 9);
}

#[test]
fn edit_result_fringe_reports_elided_regions() {
    let body: String = (1..=60).map(|n| format!("l{n}\n")).collect();
    let table = LineTable::from_string(body);
    let outcome = apply_edits(
        Path::new("big.txt"),
        &table,
        &[single(30, "l30", "L30")],
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(outcome.fringe.omitted_before, 26);
    assert_eq!(outcome.fringe.omitted_after, 27);
    assert!(
        outcome
            .fringe
            .render()
            .contains("[... 26 lines omitted ...]")
    );
    assert!(
        outcome
            .fringe
            .render()
            .contains("[... 27 lines omitted ...]")
    );
}

#[test]
fn ambiguous_old_text_is_typed_rejection_never_first_match() {
    // Two identical bodies: `    let x = 1;` matches at lines 2 and 6.
    let dup = "fn a() {\n    let x = 1;\n}\n\nfn b() {\n    let x = 1;\n}\n";
    let table = LineTable::from_string(dup.to_owned());
    let err = apply_edits(
        Path::new("dup.rs"),
        &table,
        &[single(2, "    let x = 1;", "    let x = 2;")],
        None,
    )
    .unwrap_err();
    let ToolsError::AmbiguousAnchor {
        line: 2,
        occurrences,
    } = &err
    else {
        panic!("expected AmbiguousAnchor, got {err:?}");
    };
    assert_eq!(occurrences, &vec![2, 6]);
}

#[test]
fn stale_file_is_anchor_mismatch_and_disk_untouched() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let file = dir.path().join("stale.rs");
    std::fs::write(&file, rust_source()).unwrap_or_else(|e| panic!("{e}"));
    // External modification after the model's view: line 7 rewritten.
    std::fs::write(
        &file,
        rust_source().replace("let other = 2;", "let other = 99;"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let err = apply_edits_path(
        &file,
        &[single(7, "    let other = 2;", "    let other = 20;")],
        None,
    )
    .unwrap_err();
    assert!(
        matches!(err, ToolsError::AnchorMismatch { line: 7, .. }),
        "got {err:?}"
    );
    let on_disk = std::fs::read(&file).unwrap_or_else(|e| panic!("{e}"));
    assert!(String::from_utf8_lossy(&on_disk).contains("let other = 99;"));
    assert!(!String::from_utf8_lossy(&on_disk).contains("let other = 20;"));
}

#[test]
fn hashline_drift_small_shift_reanchors_and_applies() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let file = dir.path().join("drift.rs");
    let original = rust_source();
    std::fs::write(&file, &original).unwrap_or_else(|e| panic!("{e}"));
    let table = LineTable::from_string(original.clone());
    let anchor =
        harness_tools::anchors::Hashline::compute(&table, 7).unwrap_or_else(|e| panic!("{e}"));
    // External insert of 3 lines above pushes the target from 7 to 10.
    let shifted = format!("// header 1\n// header 2\n// header 3\n{original}");
    std::fs::write(&file, shifted).unwrap_or_else(|e| panic!("{e}"));
    let edit = LineEdit {
        line: 7,
        old_text: "    let other = 2;".to_owned(),
        new_text: "    let other = 20;".to_owned(),
        hash: Some(anchor),
    };
    let outcome = apply_edits_path(&file, &[edit], None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(outcome.changed_lines, vec![10]);
    let after = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{e}"));
    assert!(after.contains("let other = 20;"));
    assert!(after.contains("// header 3"));
}

#[test]
fn hashline_drift_large_shift_is_typed_rejection() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let file = dir.path().join("bigdrift.rs");
    let original = rust_source();
    std::fs::write(&file, &original).unwrap_or_else(|e| panic!("{e}"));
    let table = LineTable::from_string(original.clone());
    let anchor =
        harness_tools::anchors::Hashline::compute(&table, 7).unwrap_or_else(|e| panic!("{e}"));
    let noise: String = (1..=400).map(|n| format!("// noise {n}\n")).collect();
    let shifted = format!("{noise}{original}");
    std::fs::write(&file, shifted).unwrap_or_else(|e| panic!("{e}"));
    let edit = LineEdit {
        line: 7,
        old_text: "    let other = 2;".to_owned(),
        new_text: "    let other = 20;".to_owned(),
        hash: Some(anchor),
    };
    let err = apply_edits_path(&file, &[edit], None).unwrap_err();
    assert!(
        matches!(err, ToolsError::AnchorDrift { anchor_line: 7, .. }),
        "got {err:?}"
    );
    let after = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{e}"));
    assert!(after.contains("let other = 2;"), "file must be untouched");
}

#[test]
fn crlf_file_lf_old_text_matches_and_crlf_is_preserved() {
    let crlf_source = rust_source().replace('\n', "\r\n");
    let table =
        LineTable::from_bytes(crlf_source.clone().into_bytes()).unwrap_or_else(|e| panic!("{e}"));
    let outcome = apply_edits(
        Path::new("win.rs"),
        &table,
        &[single(7, "    let other = 2;", "    let other = 20;")],
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let bytes = outcome.table.as_bytes();
    assert!(bytes.windows(2).any(|w| w == b"\r\n"), "CRLF must survive");
    let text = String::from_utf8_lossy(bytes);
    assert!(
        text.contains("let other = 20;\r\n"),
        "new line must be CRLF: {text:?}"
    );
    // Unchanged lines keep their exact original bytes.
    assert!(text.starts_with("fn alpha() {\r\n"));
}

#[test]
fn mixed_ending_file_edits_keep_each_range_local_ending() {
    let mixed = "one\r\ntwo\nthree\r\nfour\n".to_owned();
    let table = LineTable::from_string(mixed.clone());
    assert_eq!(table.ending(2), Some(harness_tools::LineEnding::Lf));
    let outcome = apply_edits(
        Path::new("mix.txt"),
        &table,
        &[single(2, "two", "TWO")],
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let text = outcome.table.into_string();
    assert_eq!(text, "one\r\nTWO\nthree\r\nfour\n");
}

#[test]
fn insertion_is_old_text_plus_prepended_lines() {
    let table = LineTable::from_string(rust_source());
    let insert = single(6, "fn beta() {", "    // inserted guard\nfn beta() {");
    let outcome =
        apply_edits(Path::new("ins.rs"), &table, &[insert], None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(outcome.table.content_opt(6), Some("    // inserted guard"));
    assert_eq!(outcome.table.content_opt(7), Some("fn beta() {"));
    assert_eq!(outcome.table.line_count(), 10);
}

#[test]
fn batch_edits_apply_together_and_report_new_line_numbers() {
    let table = LineTable::from_string(rust_source());
    let outcome = apply_edits(
        Path::new("batch.rs"),
        &table,
        &[
            single(2, "    let value = 1;", "    let value = 10;"),
            single(
                7,
                "    let other = 2;",
                "    let other = 20;\n    let extra = 30;",
            ),
        ],
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(outcome.changed_lines, vec![2, 7, 8]);
    assert_eq!(outcome.table.content_opt(8), Some("    let extra = 30;"));
}
