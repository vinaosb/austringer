//! Screen-delta model chaos tests (plan todo 19 acceptance): progress
//! bars, TUI redraws, ANSI floods, invalid UTF-8, scrollback capture,
//! resize, chunking invariance.

use harness_exec::screen::ScreenModel;
use harness_exec::window::window_lines;

fn feed_lines(model: &mut ScreenModel, count: usize, width_hint: usize) {
    let mut stream = String::new();
    for i in 0..count {
        stream.push_str(&format!("L{i:0width$}\r\n", width = width_hint));
    }
    model.process(stream.as_bytes());
}

#[test]
fn progress_bar_collapses_to_final_line() {
    let mut model = ScreenModel::new(80, 6);
    let mut stream = String::new();
    for percent in [7, 25, 50, 75, 99] {
        stream.push_str(&format!("\rloading {percent:3}%"));
    }
    stream.push_str("\rcomplete!   \r\n");
    model.process(stream.as_bytes());

    let view = model.finish();
    assert_eq!(view.visible, vec!["complete!".to_string()]);
    assert_eq!(view.scrolled_total, 0, "progress rewrites never scroll");
    assert!(!view.lossy);
    // No intermediate spam survives.
    let text = view.transcript_lines().join("\n");
    assert!(
        !text.contains("loading"),
        "intermediate frames must not leak: {text}"
    );
}

#[test]
fn sgr_and_curses_styling_stripped() {
    let mut model = ScreenModel::new(40, 4);
    model.process(b"\x1b[1;31mRED\x1b[0m \x1b[1mbold\x1b[22m \x1b[4munderline\x1b[24m\r\n");
    let view = model.finish();
    assert_eq!(view.visible, vec!["RED bold underline".to_string()]);
}

#[test]
fn invalid_utf8_is_skipped_not_fatal() {
    let mut model = ScreenModel::new(40, 4);
    let garbage: &[u8] = &[
        0x66, 0x6f, 0x6f, 0xff, 0xfe, 0x62, 0x61, 0x72, b'\r', b'\n', 0xc3, 0x28, 0x41, b'\r',
        b'\n',
    ];
    model.process(garbage);
    let view = model.finish();
    // No panic; the valid glyphs render; nothing raw leaks.
    assert!(view.visible.iter().any(|line| line.contains("foobar")));
    assert!(view.scrolled_total == 0 || view.scrolled_total <= 2);
}

#[test]
fn scrolled_rows_land_in_bounded_transcript() {
    let mut model = ScreenModel::new(10, 4);
    feed_lines(&mut model, 20, 2);
    let view = model.finish();
    // The trailing \n after L19 scrolls L16 off and leaves a blank prompt
    // row: 17 captures + 3 visible = all 20 lines accounted for.
    assert_eq!(view.scrolled_total, 17);
    assert_eq!(view.visible, vec!["L17", "L18", "L19"]);
    assert_eq!(
        view.scrolled,
        (0..=16).map(|i| format!("L{i:02}")).collect::<Vec<_>>()
    );
    assert!(!view.lossy);
}

#[test]
fn huge_stream_windows_to_bounded_text() {
    let mut model = ScreenModel::new(120, 40);
    feed_lines(&mut model, 200, 3);
    let view = model.finish();
    assert_eq!(view.scrolled_total, 161);
    let lines = view.transcript_lines();
    assert!(lines.len() <= 115, "composed excerpt must stay bounded");
    let true_total = view.scrolled_total + view.visible.len() as u64;
    assert_eq!(true_total, 200);
    let windowed = window_lines(&lines, true_total);
    assert_eq!(windowed.omitted_lines, 125, "200 lines - 75 window");
    assert!(windowed.text.len() < 16 * 1024);
    assert!(windowed.text.contains("[... 125 lines omitted ...]"));
    assert!(windowed.text.contains("L000"));
    assert!(windowed.text.contains("L199"));
    assert!(!windowed.text.contains("L100\n"), "middle elided");
}

#[test]
fn tui_redraw_renders_final_screen_only() {
    let mut model = ScreenModel::new(30, 4);
    model.process(b"header\r\nmenu-a\r\nmenu-b\r\n");
    model.process(b"\x1b[2J\x1b[H");
    model.process(b"\x1b[1;1HTOP\x1b[2;5HLATER");
    let view = model.finish();
    assert_eq!(view.visible, vec!["TOP", "    LATER"]);
    assert_eq!(view.scrolled_total, 0);
    assert!(!view.lossy, "clear+redraw is legitimate, not lossy");
}

#[test]
fn alternate_screen_suspends_transcript_capture() {
    let mut model = ScreenModel::new(20, 4);
    model.process(b"\x1b[?1049h");
    feed_lines(&mut model, 50, 2);
    model.process(b"\x1b[?1049l");
    model.process(b"back\r\n");
    let view = model.finish();
    assert_eq!(
        view.scrolled_total, 0,
        "alt-screen scrolling is not scrollback"
    );
    assert_eq!(view.visible, vec!["back".to_string()]);
    assert!(!view.lossy);
}

#[test]
fn resize_midstream_stays_bounded() {
    let mut model = ScreenModel::new(40, 6);
    feed_lines(&mut model, 10, 2);
    model.resize(20, 3);
    feed_lines(&mut model, 10, 2);
    let view = model.finish();
    assert!(view.visible.len() <= 3);
    assert!(view.transcript_lines().len() <= 115);
}

#[test]
fn cursor_tracking_matches_parser() {
    let mut model = ScreenModel::new(20, 4);
    model.process(b"ab\tc\x08d");
    // a(0,1) b(0,2) TAB->(0,8) c(0,9) BS->(0,8) d(0,9)
    assert_eq!(model.cursor(), (0, 9));
    model.process(b"\r\nnext");
    assert_eq!(model.cursor(), (1, 4));
}

#[test]
fn long_unbroken_word_wraps_without_losing_text() {
    let mut model = ScreenModel::new(10, 4);
    model.process(b"ABCDEFGHIJKLM");
    let view = model.finish();
    // vt100 joins wrapped rows into one logical line.
    assert_eq!(view.visible, vec!["ABCDEFGHIJKLM".to_string()]);
    assert_eq!(view.scrolled_total, 0);
}

#[test]
fn binary_flood_never_panics_or_grows() {
    let mut model = ScreenModel::new(60, 10);
    let mut state = 0x1234_5678_9abc_u64;
    let mut flood = Vec::with_capacity(100_000);
    for _ in 0..100_000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        flood.push((state >> 33) as u8);
    }
    model.process(&flood);
    let view = model.finish();
    assert!(view.transcript_lines().len() <= 115);
}

#[test]
fn ansi_escape_flood_stays_bounded() {
    let mut model = ScreenModel::new(60, 6);
    let mut flood = String::new();
    for i in 0..5_000 {
        flood.push_str(&format!(
            "\x1b[{};{}H\x1b[3{}mX",
            i % 6 + 1,
            i % 60 + 1,
            i % 8
        ));
    }
    model.process(flood.as_bytes());
    let view = model.finish();
    assert!(view.visible.len() <= 6);
    assert!(view.transcript_lines().len() <= 115);
}

#[test]
fn repeated_identical_lines_capture_exact_count() {
    let mut model = ScreenModel::new(10, 4);
    let mut stream = String::new();
    for _ in 0..100 {
        stream.push_str("SAME\r\n");
    }
    model.process(stream.as_bytes());
    let view = model.finish();
    // 100 lines => 97 captures + 3 visible (trailing blank trimmed).
    assert_eq!(view.scrolled_total, 97);
    assert_eq!(view.scrolled.len(), 75);
    assert_eq!(view.visible.len(), 3);
    assert!(!view.lossy);
}

#[test]
fn chunking_invariance_whole_vs_piecewise() {
    let mut stream = String::new();
    for i in 0..60 {
        stream.push_str(&format!("line-{i:03} text here\r\n"));
    }
    let mut whole = ScreenModel::new(30, 5);
    whole.process(stream.as_bytes());
    let mut piecewise = ScreenModel::new(30, 5);
    let bytes = stream.as_bytes();
    let mut offset = 0;
    for size in [1, 3, 7, 2, 11, 5] {
        let end = (offset + size).min(bytes.len());
        piecewise.process(&bytes[offset..end]);
        offset = end;
        if offset >= bytes.len() {
            break;
        }
    }
    piecewise.process(&bytes[offset.min(bytes.len())..]);
    assert_eq!(whole.finish(), piecewise.finish());
}

#[test]
fn fresh_model_is_blank_per_command() {
    // Structural stale-state guard: each command gets a new ScreenModel.
    let mut first = ScreenModel::new(30, 4);
    first.process(b"FIRST COMMAND OUTPUT\r\n");
    let first_view = first.finish();
    let second = ScreenModel::new(30, 4);
    let second_view = second.finish();
    assert!(
        first_view
            .visible
            .contains(&"FIRST COMMAND OUTPUT".to_string())
    );
    assert!(second_view.visible.is_empty());
    assert_eq!(second_view.scrolled_total, 0);
}
