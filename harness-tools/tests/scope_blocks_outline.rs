//! Stage-D tests: strict read-scope ledger, indentation-anchored block
//! reads (Roo), and the definitions-only outline mode.

use harness_tools::blocks::read_block;
use harness_tools::outline::{OutlineOutcome, outline};
use harness_tools::strict::{MemoryLedger, ReadLedger, enforce_read_scope};
use harness_tools::{LineTable, ToolsError};
use std::path::Path;

fn source() -> String {
    [
        "fn alpha() {",
        "    let a = 1;",
        "    let b = 2;",
        "}",
        "",
        "fn beta() {",
        "    let c = 3;",
        "    let d = 4;",
        "}",
        "",
        "fn gamma() {",
        "    let e = 5;",
        "}",
    ]
    .join("\n")
        + "\n"
}

#[test]
fn ledger_records_windows_and_covers_exactly() {
    let mut ledger = MemoryLedger::new();
    ledger.record_read(Path::new("a.rs"), 5, 20);
    assert!(ledger.covers(Path::new("a.rs"), &(10..=10)));
    assert!(ledger.covers(Path::new("a.rs"), &(5..=20)));
    assert!(!ledger.covers(Path::new("a.rs"), &(4..=10)));
    assert!(!ledger.covers(Path::new("a.rs"), &(5..=21)));
    assert!(!ledger.covers(Path::new("other.rs"), &(1..=1)));
}

#[test]
fn ledger_merges_adjacent_and_separate_windows() {
    let mut ledger = MemoryLedger::new();
    ledger.record_read(Path::new("a.rs"), 1, 10);
    ledger.record_read(Path::new("a.rs"), 15, 20);
    ledger.record_read(Path::new("a.rs"), 11, 14);
    assert_eq!(ledger.read_windows(Path::new("a.rs")), vec![(1, 20)]);
    assert!(!ledger.covers(Path::new("a.rs"), &(20..=25)));
}

#[test]
fn strict_mode_rejects_edit_to_never_read_file() {
    let ledger = MemoryLedger::new();
    let edit = 7..=9;
    let err = enforce_read_scope(&ledger, Path::new("secret.rs"), std::slice::from_ref(&edit))
        .unwrap_err();
    let ToolsError::UnreadRegion { path, missing } = &err else {
        panic!("expected UnreadRegion, got {err:?}");
    };
    assert!(path.ends_with("secret.rs"));
    assert_eq!(missing, &[edit]);
}

#[test]
fn strict_mode_rejects_edit_to_unread_span_of_read_file() {
    let mut ledger = MemoryLedger::new();
    ledger.record_read(Path::new("a.rs"), 5, 20);
    let edit = 18..=24;
    let err = enforce_read_scope(&ledger, Path::new("a.rs"), &[edit]).unwrap_err();
    let ToolsError::UnreadRegion { missing, .. } = &err else {
        panic!("expected UnreadRegion, got {err:?}");
    };
    let expected = 21..=24;
    assert_eq!(missing, &[expected]);
    let covered = 18..=20;
    assert!(enforce_read_scope(&ledger, Path::new("a.rs"), &[covered]).is_ok());
}

#[test]
fn strict_mode_reports_all_missing_subranges() {
    let mut ledger = MemoryLedger::new();
    ledger.record_read(Path::new("a.rs"), 1, 10);
    ledger.record_read(Path::new("a.rs"), 15, 20);
    let edit = 8..=18;
    let err = enforce_read_scope(&ledger, Path::new("a.rs"), &[edit]).unwrap_err();
    let ToolsError::UnreadRegion { missing, .. } = &err else {
        panic!("expected UnreadRegion, got {err:?}");
    };
    let expected = 11..=14;
    assert_eq!(missing, &[expected]);
}

#[test]
fn block_read_anchor_on_header_expands_to_whole_function() {
    let table = LineTable::from_string(source());
    let block = read_block(Path::new("a.rs"), &table, 6).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((block.first_line, block.last_line), (6, 9));
    assert!(block.view.render().contains("fn beta() {"));
    assert!(block.view.render().contains("    let d = 4;"));
}

#[test]
fn block_read_anchor_inside_body_expands_up_to_header() {
    let table = LineTable::from_string(source());
    let block = read_block(Path::new("a.rs"), &table, 3).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((block.first_line, block.last_line), (1, 4));
}

#[test]
fn block_read_stops_at_sibling_boundary() {
    let table = LineTable::from_string(source());
    let block = read_block(Path::new("a.rs"), &table, 2).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(block.last_line, 4, "must not leak into fn beta");
    assert!(!block.view.render().contains("fn beta"));
}

#[test]
fn block_read_blank_anchor_is_typed_rejection() {
    let table = LineTable::from_string(source());
    let err = read_block(Path::new("a.rs"), &table, 5).unwrap_err();
    assert!(
        matches!(err, ToolsError::BlankAnchor { line: 5 }),
        "got {err:?}"
    );
}

#[test]
fn block_read_out_of_range_is_typed() {
    let table = LineTable::from_string(source());
    let err = read_block(Path::new("a.rs"), &table, 99).unwrap_err();
    assert!(
        matches!(err, ToolsError::LineOutOfRange { line: 99, .. }),
        "got {err:?}"
    );
}

#[test]
fn block_read_tab_indented_blocks_expand() {
    let text = "def one():\n\tbody = 1\n\treturn body\n\ndef two():\n\treturn 2\n";
    let table = LineTable::from_string(text.to_owned());
    let block = read_block(Path::new("m.py"), &table, 5).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((block.first_line, block.last_line), (5, 6));
}

#[test]
fn block_read_oversized_block_is_windowed_with_placeholder() {
    let mut text = String::from("fn huge() {\n");
    for n in 0..150 {
        text.push_str(&format!("    let v{n} = {n};\n"));
    }
    text.push_str("}\n");
    let table = LineTable::from_string(text);
    let block = read_block(Path::new("huge.rs"), &table, 1).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(block.first_line, 1);
    assert_eq!(block.last_line, 152);
    assert_eq!(block.view.last_line, 100);
    assert!(block.view.render().contains("lines omitted"));
}

#[test]
fn outline_lists_definitions_with_line_kind_name() {
    let path = Path::new("a.rs");
    let outcome = outline(path, source().as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    let OutlineOutcome::Outline(view) = outcome else {
        panic!("expected outline for a .rs file");
    };
    let rendered = view.render.as_str();
    assert!(rendered.contains("     1 fn        alpha"), "{rendered}");
    assert!(rendered.contains("     6 fn        beta"), "{rendered}");
    assert!(rendered.contains("    11 fn        gamma"), "{rendered}");
    assert_eq!(view.rows.len(), 3);
}

#[test]
fn outline_unknown_extension_degrades_to_no_grammar() {
    let outcome = outline(Path::new("notes.zzz"), b"hello\n").unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(outcome, OutlineOutcome::NoGrammar { .. }));
}

#[test]
fn outline_invalid_utf8_is_typed() {
    let err = outline(Path::new("x.rs"), &[0xff, 0xfe]).unwrap_err();
    assert!(matches!(err, ToolsError::NonUtf8 { .. }), "got {err:?}");
}

#[test]
fn outline_unicode_identifiers_render() {
    let text = "fn π() {}\nstruct Ω { x: u8 }\n";
    let outcome = outline(Path::new("u.rs"), text.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    let OutlineOutcome::Outline(view) = outcome else {
        panic!("expected outline");
    };
    assert!(view.render.as_str().contains("π"));
    assert!(view.render.as_str().contains("Ω"));
    assert!(view.render.as_str().contains("struct"));
}
