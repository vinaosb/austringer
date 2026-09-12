//! The windowed output law (plan todo 19): >75 lines => first 25 + last 50
//! + omission marker. The model never receives unbounded transcripts.

use harness_exec::window::{HEAD_LINES, TAIL_LINES, WINDOW_LIMIT, window_lines};

fn lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("line-{i:04}")).collect()
}

#[test]
fn window_passes_short_transcripts_through_verbatim() {
    let given = lines(10);
    let windowed = window_lines(&given, 10);
    assert_eq!(windowed.text, given.join("\n"));
    assert_eq!(windowed.total_lines, 10);
    assert_eq!(windowed.omitted_lines, 0);
}

#[test]
fn window_keeps_exactly_75_lines_intact() {
    let given = lines(WINDOW_LIMIT);
    let windowed = window_lines(&given, WINDOW_LIMIT as u64);
    assert_eq!(windowed.omitted_lines, 0);
    assert_eq!(windowed.text, given.join("\n"));
}

#[test]
fn window_applies_head_tail_marker_at_76_lines() {
    let given = lines(WINDOW_LIMIT + 1);
    let windowed = window_lines(&given, (WINDOW_LIMIT + 1) as u64);
    assert_eq!(windowed.total_lines, (WINDOW_LIMIT + 1) as u64);
    assert_eq!(windowed.omitted_lines, 1);

    let rendered = windowed.text.split('\n').collect::<Vec<_>>();
    assert_eq!(rendered.len(), HEAD_LINES + 1 + TAIL_LINES);
    // First 25 lines are the true head.
    for (i, rendered_line) in rendered[..HEAD_LINES].iter().enumerate() {
        assert_eq!(*rendered_line, format!("line-{i:04}"));
    }
    // Omission marker names the elided count.
    assert_eq!(rendered[HEAD_LINES], "[... 1 lines omitted ...]");
    // Last 50 lines are the true tail of the transcript.
    let tail_start = WINDOW_LIMIT + 1 - TAIL_LINES;
    for (offset, rendered_line) in rendered[HEAD_LINES + 1..].iter().enumerate() {
        assert_eq!(
            *rendered_line,
            format!("line-{:04}", tail_start + offset),
            "tail line {offset}"
        );
    }
}

#[test]
fn window_marker_grows_with_transcript() {
    let given = lines(10_000);
    let windowed = window_lines(&given, 10_000);
    assert_eq!(windowed.omitted_lines, (10_000 - WINDOW_LIMIT) as u64);
    assert!(windowed.text.contains("[... 9925 lines omitted ...]"));
    // The bounded text stays small even for huge transcripts.
    assert!(windowed.text.len() < 16 * 1024);
}

#[test]
fn window_reports_virtual_totals_for_excerpt_lines() {
    // The runner passes a bounded excerpt (head 25 + tail 50) while the
    // true transcript was far larger: the marker must name the real count.
    let mut excerpt = lines(HEAD_LINES);
    excerpt.extend((9_950..10_000).map(|i| format!("line-{i:04}")));
    let windowed = window_lines(&excerpt, 10_000);
    assert_eq!(windowed.omitted_lines, 9_925);
    assert!(windowed.text.starts_with("line-0000\n"));
    assert!(
        windowed
            .text
            .contains("line-0024\n[... 9925 lines omitted ...]\nline-9950")
    );
    assert!(windowed.text.ends_with("line-9999"));
    assert!(windowed.text.len() < 16 * 1024);
}

#[test]
fn window_empty_transcript_is_empty() {
    let windowed = window_lines(&[], 0);
    assert_eq!(windowed.text, "");
    assert_eq!(windowed.omitted_lines, 0);
}
