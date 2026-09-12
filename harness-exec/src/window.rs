//! The windowed output law (plan todo 19): bounded model-facing text.
//!
//! More than [`WINDOW_LIMIT`] lines never reaches the model — the law
//! renders the first [`HEAD_LINES`] plus the last [`TAIL_LINES`] with an
//! explicit omission marker in between, so elided output is always
//! announced, never silent. `lines` may be an excerpt (head + tail +
//! visible screen) of a longer transcript; `total_lines` is the true
//! logical line count, so the marker names the real elision.

/// Lines shown from the top of the transcript.
pub const HEAD_LINES: usize = 25;

/// Lines shown from the bottom of the transcript.
pub const TAIL_LINES: usize = 50;

/// Above this many lines the window applies.
pub const WINDOW_LIMIT: usize = HEAD_LINES + TAIL_LINES;

/// The windowed rendering of a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Windowed {
    /// The bounded, model-facing text.
    pub text: String,
    /// Total logical lines before windowing.
    pub total_lines: u64,
    /// Lines elided by the window (0 when everything fit).
    pub omitted_lines: u64,
}

/// Applies the window law. `lines` is the bounded excerpt, `total_lines`
/// the true transcript length; callers with the complete transcript pass
/// `lines.len()`.
#[must_use]
pub fn window_lines(lines: &[String], total_lines: u64) -> Windowed {
    let total = total_lines.max(lines.len() as u64);
    if total <= WINDOW_LIMIT as u64 {
        return Windowed {
            text: lines.join("\n"),
            total_lines: total,
            omitted_lines: 0,
        };
    }
    let head_end = lines.len().min(HEAD_LINES);
    let head = &lines[..head_end];
    let tail_start = lines.len().saturating_sub(TAIL_LINES).max(head_end);
    let tail = &lines[tail_start..];
    let shown = (head.len() + tail.len()) as u64;
    let omitted = total.saturating_sub(shown);
    let mut text = String::with_capacity(
        head.iter().map(|line| line.len() + 1).sum::<usize>()
            + tail.iter().map(|line| line.len() + 1).sum::<usize>()
            + 32,
    );
    for line in head {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(&format!("[... {omitted} lines omitted ...]\n"));
    for line in tail {
        text.push_str(line);
        text.push('\n');
    }
    text.pop();
    Windowed {
        text,
        total_lines: total,
        omitted_lines: omitted,
    }
}
