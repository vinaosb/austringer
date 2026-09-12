//! Adversarial edit_lines inputs (plan todo 16): malformed payloads,
//! structural caps, overlaps, out-of-range anchors, unicode/tab/emoji
//! lines - every one a typed rejection or a clean application.

use harness_tools::edit::{LineEdit, MAX_EDITS_PER_CALL, MAX_NEW_LINES_PER_EDIT, apply_edits};
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
fn malformed_inputs_are_typed_rejections() {
    let table = LineTable::from_string(rust_source());
    let no_edits: Vec<LineEdit> = Vec::new();
    assert!(matches!(
        apply_edits(Path::new("x"), &table, &no_edits, None),
        Err(ToolsError::NoEdits)
    ));
    assert!(matches!(
        apply_edits(Path::new("x"), &table, &[single(2, "", "y")], None),
        Err(ToolsError::EmptyOldText { line: 2 })
    ));
    let too_many: Vec<LineEdit> = (0..=MAX_EDITS_PER_CALL)
        .map(|n| single(2 + n, &format!("    let value = {n};"), "x"))
        .collect();
    // Only 2 of these actually match; build a file where they all match
    // instead: simpler to test the cap with matching content is unnecessary
    // â€” the cap fires before matching.
    assert!(matches!(
        apply_edits(Path::new("x"), &table, &too_many, None),
        Err(ToolsError::TooManyEdits { count, max }) if count == MAX_EDITS_PER_CALL + 1 && max == MAX_EDITS_PER_CALL
    ));
    let huge_new: String = (0..=MAX_NEW_LINES_PER_EDIT)
        .map(|n| format!("{n}\n"))
        .collect();
    assert!(matches!(
        apply_edits(
            Path::new("x"),
            &table,
            &[LineEdit {
                line: 2,
                old_text: "    let value = 1;".to_owned(),
                new_text: huge_new,
                hash: None
            }],
            None
        ),
        Err(ToolsError::EditTooLarge { .. })
    ));
    let ok_pair = apply_edits(
        Path::new("x"),
        &table,
        &[
            single(2, "    let value = 1;", "a"),
            single(3, "    println!(\"{value}\");", "b"),
        ],
        None,
    );
    assert!(ok_pair.is_ok());
    // Overlapping edits (same anchor) are rejected atomically.
    assert!(matches!(
        apply_edits(
            Path::new("x"),
            &table,
            &[
                single(2, "    let value = 1;", "a"),
                single(2, "    let value = 1;\n    println!(\"{value}\");", "b")
            ],
            None
        ),
        Err(ToolsError::OverlappingEdits { .. })
    ));
    // Out-of-range anchor line.
    assert!(matches!(
        apply_edits(Path::new("x"), &table, &[single(99, "nope", "x")], None),
        Err(ToolsError::AnchorMismatch { .. })
    ));
}

#[test]
fn unicode_tabs_and_emoji_lines_edit_cleanly() {
    let source = "fn Ï€() {\n\tlet s = \"æ—¥æœ¬èªž ðŸ˜€\";\n}\n".to_owned();
    let table = LineTable::from_string(source);
    let outcome = apply_edits(
        Path::new("uni.rs"),
        &table,
        &[single(
            2,
            "\tlet s = \"æ—¥æœ¬èªž ðŸ˜€\";",
            "\tlet s = \"áºž ðŸ˜€\";",
        )],
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        outcome.table.content_opt(2),
        Some("\tlet s = \"áºž ðŸ˜€\";")
    );
}
#[test]
fn syntax_gate_rejects_broken_edit_with_diagnostic() {
    let table = LineTable::from_string(rust_source());
    // Remove the closing brace of alpha -> new ERROR node appears.
    let err = apply_edits(
        Path::new("gate.rs"),
        &table,
        &[single(8, "    other + 1\n}", "    other + 1")],
        Some(&harness_tools::gate::SyntaxGate::for_path(Path::new(
            "gate.rs",
        ))),
    )
    .unwrap_err();
    let ToolsError::SyntaxGate {
        count, diagnostics, ..
    } = &err
    else {
        panic!("expected SyntaxGate, got {err:?}");
    };
    assert!(*count >= 1);
    assert!(!diagnostics.is_empty(), "diagnostic must be attached");
}

#[test]
fn syntax_gate_passes_valid_edits_and_skips_without_grammar() {
    let table = LineTable::from_string(rust_source());
    let ok = apply_edits(
        Path::new("gate.rs"),
        &table,
        &[single(2, "    let value = 1;", "    let value = 11;")],
        Some(&harness_tools::gate::SyntaxGate::for_path(Path::new(
            "gate.rs",
        ))),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ok.gate, harness_tools::gate::GateOutcome::Passed);

    let plain = LineTable::from_string("line one\nline two\n".to_owned());
    let skipped = apply_edits(
        Path::new("notes.zzz"),
        &plain,
        &[single(2, "line two", "LINE TWO")],
        Some(&harness_tools::gate::SyntaxGate::for_path(Path::new(
            "notes.zzz",
        ))),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        skipped.gate,
        harness_tools::gate::GateOutcome::SkippedNoGrammar
    );
    assert_eq!(skipped.table.content_opt(2), Some("LINE TWO"));
}

#[test]
fn syntax_gate_allows_edits_on_already_broken_files_when_not_worsening() {
    let broken = "fn a() {\n    let x = 1;\n".to_owned() + "fn b() {\n}\n";
    let table = LineTable::from_string(broken);
    // Pre-existing error (unclosed a) must not block an unrelated clean edit.
    let outcome = apply_edits(
        Path::new("broken.rs"),
        &table,
        &[single(3, "fn b() {", "fn bee() {")],
        Some(&harness_tools::gate::SyntaxGate::for_path(Path::new(
            "gate.rs",
        ))),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(outcome.gate, harness_tools::gate::GateOutcome::Passed);
}
