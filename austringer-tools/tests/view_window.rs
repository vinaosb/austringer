//! Stage-A behavioral tests: CRLF-aware line table + bounded view windows
//! (plan todo 16, clauses 1/10/12 + CRLF line-math + adversarial inputs).

use austringer_tools::window::{MAX_WINDOW_LINES, view_window};
use austringer_tools::{LineEnding, LineTable, ToolsError};
use std::path::Path;

fn lf(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

fn crlf(s: &str) -> Vec<u8> {
    s.replace("\n", "\r\n").into_bytes()
}

#[test]
fn line_table_splits_lf_and_counts_without_final_newline() {
    let table = LineTable::from_bytes(lf("alpha\nbeta\ngamma")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(table.line_count(), 3);
    assert_eq!(table.content_opt(1), Some("alpha"));
    assert_eq!(table.content_opt(3), Some("gamma"));
    assert_eq!(table.content_opt(4), None);
    assert_eq!(table.ending(3), Some(LineEnding::Eof));
    assert_eq!(table.ending(1), Some(LineEnding::Lf));
}

#[test]
fn line_table_is_crlf_correct_for_windows_files() {
    let table = LineTable::from_bytes(crlf("alpha\nbeta\n")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(table.line_count(), 2);
    // Content must exclude the terminator; line 2 is empty before its CRLF.
    assert_eq!(table.content_opt(1), Some("alpha"));
    assert_eq!(table.content_opt(2), Some("beta"));
    assert_eq!(table.ending(1), Some(LineEnding::Crlf));
    assert_eq!(table.ending(2), Some(LineEnding::Crlf));
    assert_eq!(table.dominant_ending(), LineEnding::Crlf);
    // Round-trip preserves the original bytes exactly.
    assert_eq!(table.as_bytes(), crlf("alpha\nbeta\n"));
}

#[test]
fn line_table_handles_mixed_crlf_lf_and_lone_cr_content() {
    // Mixed terminators (git autocrlf artifacts) and a \r inside a line.
    let bytes = b"a\r\nb\nc\r\r\nd".to_vec();
    let table = LineTable::from_bytes(bytes.clone()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(table.line_count(), 4);
    assert_eq!(table.content_opt(3), Some("c\r"));
    assert_eq!(table.as_bytes(), bytes);
}

#[test]
fn line_table_empty_file_has_zero_lines() {
    let table = LineTable::from_bytes(Vec::new()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(table.line_count(), 0);
    assert_eq!(table.as_bytes(), Vec::<u8>::new());
}

#[test]
fn line_table_rejects_invalid_utf8_typed() {
    let err = LineTable::from_bytes(vec![0xff, 0xfe, b'\n']).unwrap_err();
    assert!(matches!(err, ToolsError::NonUtf8 { .. }), "got {err:?}");
}

#[test]
fn line_table_maps_byte_offsets_to_lines() {
    let table = LineTable::from_bytes(crlf("alpha\nbeta\ngamma")).unwrap_or_else(|e| panic!("{e}"));
    // Byte 0 -> line 1; byte 7 ('b' after CRLF) -> line 2; EOF -> line 3.
    assert_eq!(table.line_of_byte(0), 1);
    assert_eq!(table.line_of_byte(7), 2);
    assert_eq!(table.line_of_byte(table.as_bytes().len()), 3);
}

#[test]
fn window_shows_first_100_lines_of_oversized_file_with_context_header() {
    let body: String = (1..=250).map(|n| format!("line-{n}\n")).collect();
    let view =
        view_window(Path::new("src/big.rs"), &lf(&body), None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(view.total_lines, 250);
    assert_eq!(view.first_line, 1);
    assert_eq!(view.last_line, MAX_WINDOW_LINES);
    assert_eq!(view.omitted_before, 0);
    assert_eq!(view.omitted_after, 150);
    let rendered = view.render();
    assert!(
        rendered.contains("src/big.rs"),
        "header must carry the path"
    );
    assert!(
        rendered.contains("lines 1-100 of 250"),
        "context header:\n{rendered}"
    );
    assert!(rendered.contains("  1 | line-1"));
    assert!(rendered.contains("100 | line-100"));
    // Anti-elision guard: the 150 hidden lines are an explicit placeholder,
    // never silent.
    assert!(
        rendered.contains("[... 150 lines omitted ...]"),
        "{rendered}"
    );
    assert!(!rendered.contains("line-101"));
}

#[test]
fn window_jump_to_line_beyond_start_is_clamped_to_file_tail() {
    let body: String = (1..=120).map(|n| format!("l{n}\n")).collect();
    let view = view_window(Path::new("f"), &lf(&body), Some(110)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(view.first_line, 110);
    assert_eq!(view.last_line, 120);
    assert_eq!(view.omitted_before, 109);
    assert_eq!(view.omitted_after, 0);
    assert!(view.render().contains("[... 109 lines omitted ...]"));
}

#[test]
fn window_jump_out_of_range_is_typed_rejection() {
    let err = view_window(Path::new("f"), &lf("a\nb\n"), Some(4)).unwrap_err();
    assert!(
        matches!(err, ToolsError::LineOutOfRange { line: 4, total: 2 }),
        "got {err:?}"
    );
}

#[test]
fn window_of_crlf_file_renders_content_without_terminators() {
    let view = view_window(Path::new("win.txt"), &crlf("one\ntwo\n"), None)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(view.total_lines, 2);
    let rendered = view.render();
    assert!(rendered.contains("one"));
    assert!(
        !rendered.contains('\r'),
        "CRLF must not leak into the model view"
    );
}

#[test]
fn window_empty_file_is_reported_not_panicked() {
    let view = view_window(Path::new("empty.txt"), &[], None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(view.total_lines, 0);
    assert!(view.render().contains("empty.txt"));
}

#[test]
fn window_invalid_utf8_is_typed_rejection() {
    let err = view_window(Path::new("bin"), &[0xC3, 0x28, b'\n'], None).unwrap_err();
    assert!(matches!(err, ToolsError::NonUtf8 { .. }), "got {err:?}");
}

#[test]
fn window_small_file_has_no_omission_placeholders() {
    let view =
        view_window(Path::new("s.txt"), &lf("only\n"), None).unwrap_or_else(|e| panic!("{e}"));
    let rendered = view.render();
    assert!(!rendered.contains("omitted"), "{rendered}");
    assert!(rendered.contains("only"));
}

#[test]
fn window_unicode_emoji_and_tabs_render_verbatim() {
    let line = "\tsome\tÜnïcödé 😀 path ← tabs";
    let view = view_window(
        Path::new("uni→code.txt"),
        lf(&format!("{line}\n")).as_slice(),
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(view.render().contains(line));
}

#[test]
fn window_path_wrapper_reads_real_file_and_missing_path_is_typed_io() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let file = dir.path().join("real.txt");
    std::fs::write(&file, b"on disk\n").unwrap_or_else(|e| panic!("{e}"));
    let view =
        austringer_tools::window::view_window_path(&file, None).unwrap_or_else(|e| panic!("{e}"));
    assert!(view.render().contains("on disk"));

    let missing = dir.path().join("nope.txt");
    let err = austringer_tools::window::view_window_path(&missing, None).unwrap_err();
    assert!(matches!(err, ToolsError::Io(_)), "got {err:?}");
}

#[test]
fn window_overlong_windows_path_never_panics() {
    // Adversarial: >260-char Windows path. Degrades to a typed error (or
    // succeeds when the OS has long paths enabled) — either way, no panic
    // and no non-typed failure.
    let deep = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let mut path = deep.path().to_path_buf();
    for _ in 0..12 {
        path.push("very-long-directory-name-padding-the-path-length-out");
    }
    path.set_file_name("leaf.txt");
    assert!(path.as_os_str().len() > 260, "test needs a >260 char path");
    let outcome = austringer_tools::window::view_window_path(&path, None);
    assert!(
        outcome.is_ok() || matches!(outcome, Err(ToolsError::Io(_))),
        "got {outcome:?}"
    );
    drop(deep); // keep the tempdir alive until after the call
}
