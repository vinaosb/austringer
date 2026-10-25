//! Streaming markdown renderer: chunk-fed text rendered to styled
//! terminal lines. Idempotent — rendering the accumulated text always
//! yields the same lines regardless of chunk boundaries.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

fn heading_style() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

fn code_style() -> Style {
    Style::default().fg(ratatui::style::Color::Yellow)
}

fn quote_style() -> Style {
    Style::default().add_modifier(Modifier::ITALIC)
}

fn list_style() -> Style {
    Style::default().fg(ratatui::style::Color::Green)
}

fn plain_style() -> Style {
    Style::default()
}

/// Render accumulated markdown to styled lines. Empty lines are
/// dropped so a stream chunk boundary ending in `\n` never renders a
/// phantom row (rendering is chunk-boundary-idempotent by
/// construction: output depends only on the accumulated text).
#[must_use]
pub fn render(text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_fence = false;
    for raw in text.lines() {
        if raw.is_empty() {
            continue;
        }
        let styled = |style: Style| Line::from(Span::styled(raw.to_owned(), style));
        if raw.starts_with("```") {
            in_fence = !in_fence;
            lines.push(styled(code_style()));
        } else if in_fence {
            lines.push(styled(code_style()));
        } else if let Some(heading) = strip_heading(raw) {
            lines.push(Line::from(Span::styled(
                heading.to_owned(),
                heading_style(),
            )));
        } else if let Some(quote) = raw.strip_prefix("> ") {
            lines.push(Line::from(Span::styled(quote.to_owned(), quote_style())));
        } else if let Some(item) = list_item(raw) {
            lines.push(Line::from(Span::styled(item, list_style())));
        } else {
            lines.push(styled(plain_style()));
        }
    }
    lines
}

fn strip_heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ') {
        Some(&line[hashes + 1..])
    } else {
        None
    }
}

fn list_item(line: &str) -> Option<String> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    rest.starts_with("- ")
        .then(|| format!("{}• {}", &line[..indent], &rest[2..]))
}

/// Incrementally-fed markdown document.
#[derive(Debug, Clone, Default)]
pub struct StreamDoc {
    text: String,
}

impl StreamDoc {
    /// New empty document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one streaming chunk.
    pub fn push_str(&mut self, chunk: &str) {
        self.text.push_str(chunk);
    }

    /// Render the accumulated text.
    #[must_use]
    pub fn lines(&self) -> Vec<Line<'static>> {
        render(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|s| s.content.to_string())
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn headings_render_bold_and_leveled() {
        let lines = render("# Title\n## Sub\n### Deep");
        assert_eq!(plain(&lines[0]), "Title");
        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(plain(&lines[1]), "Sub");
        assert_eq!(plain(&lines[2]), "Deep");
    }

    #[test]
    fn fenced_code_lines_share_code_style() {
        let text = "before\n```rust\nlet x = 1;\n```\nafter";
        let lines = render(text);
        assert_eq!(plain(&lines[0]), "before");
        assert_eq!(plain(&lines[1]), "```rust");
        assert_eq!(plain(&lines[2]), "let x = 1;");
        assert_eq!(lines[2].spans[0].style, code_style());
        assert_eq!(plain(&lines[3]), "```");
        assert_eq!(plain(&lines[4]), "after");
        assert_ne!(lines[4].spans[0].style, code_style());
    }

    #[test]
    fn lists_and_quotes_render_indented() {
        let lines = render("- one\n  - nested\n1. first\n> quoted");
        assert_eq!(plain(&lines[0]), "• one");
        assert_eq!(plain(&lines[1]), "  • nested");
        assert_eq!(plain(&lines[2]), "1. first");
        assert_eq!(plain(&lines[3]), "quoted");
        assert!(
            lines[3].spans[0]
                .style
                .add_modifier
                .contains(Modifier::ITALIC)
        );
    }

    #[test]
    fn streaming_chunks_render_same_as_whole_text() {
        let whole = "# Plan\nbody text\n- a\n- b\n```js\ncode();\n```\ntail";
        let mut doc = StreamDoc::new();
        for chunk in [
            "# Plan\nbody",
            " text\n- a\n",
            "- b\n```js\ncod",
            "e();\n```\ntail",
        ] {
            doc.push_str(chunk);
        }
        let streamed: Vec<String> = doc.lines().iter().map(plain).collect();
        let direct: Vec<String> = render(whole).iter().map(plain).collect();
        assert_eq!(streamed, direct);
    }

    #[test]
    fn empty_text_renders_nothing() {
        assert!(render("").is_empty());
    }

    #[test]
    fn styles_differ_between_kinds() {
        let lines = render("# h\n- l\n> q\np");
        assert_eq!(lines[0].spans[0].style, heading_style());
        assert_ne!(lines[1].spans[0].style, plain_style());
        assert_eq!(lines[3].spans[0].style, plain_style());
        let _ = quote_style();
    }
}
