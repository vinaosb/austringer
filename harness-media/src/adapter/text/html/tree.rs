//! The canonical html census tree: the bounded structural summary BOTH
//! the extractor (tokenizing real bytes) and the independent spec->tree
//! projection produce. Same shape, two builders, separately tested.

use std::collections::BTreeMap;

use serde::Serialize;

use super::spec::{HtmlBlock, HtmlSpec};
use crate::adapter::text::collapse;

/// Upper bound on distinct tag names in the census (bounded map).
pub const MAX_TAGS: usize = 1024;

/// The bounded html census: doctype presence, start-tag occurrences,
/// first `<title>` text, and the collapsed character data.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HtmlTree {
    /// Occurrences per start-tag name (lowercased; void and
    /// self-closing included).
    pub census: BTreeMap<String, usize>,
    /// Whether a doctype token was seen.
    pub doctype: bool,
    /// All character data, whitespace-collapsed pieces joined with
    /// single spaces, document order, bounded.
    pub text: String,
    /// First `<title>` content (omitted when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// The INDEPENDENT spec->tree projection: the census the generated
/// document must tokenize to, computed without maud and without the
/// tokenizer.
#[must_use]
pub fn spec_tree(spec: &HtmlSpec) -> HtmlTree {
    let mut tree = HtmlTree { doctype: true, ..HtmlTree::default() };
    increment(&mut tree.census, "html");
    increment(&mut tree.census, "head");
    increment(&mut tree.census, "meta");
    if let Some(title) = &spec.title {
        increment(&mut tree.census, "title");
        tree.title = Some(collapse(title));
    }
    increment(&mut tree.census, "body");
    let mut pieces: Vec<String> = Vec::new();
    for block in &spec.blocks {
        match block {
            HtmlBlock::Heading { level, text } => {
                increment(&mut tree.census, &format!("h{}", (*level).clamp(1, 6)));
                pieces.push(collapse(text));
            }
            HtmlBlock::Paragraph { text } => {
                increment(&mut tree.census, "p");
                pieces.push(collapse(text));
            }
            HtmlBlock::List { ordered, items } => {
                increment(&mut tree.census, if *ordered { "ol" } else { "ul" });
                for item in items {
                    increment(&mut tree.census, "li");
                    pieces.push(collapse(item));
                }
            }
            HtmlBlock::Code { text } => {
                increment(&mut tree.census, "pre");
                increment(&mut tree.census, "code");
                pieces.push(collapse(text));
            }
            HtmlBlock::Table { header, rows } => {
                increment(&mut tree.census, "table");
                increment(&mut tree.census, "thead");
                increment(&mut tree.census, "tr");
                for cell in header {
                    increment(&mut tree.census, "th");
                    pieces.push(collapse(cell));
                }
                increment(&mut tree.census, "tbody");
                for row in rows {
                    increment(&mut tree.census, "tr");
                    for cell in row {
                        increment(&mut tree.census, "td");
                        pieces.push(collapse(cell));
                    }
                }
            }
        }
    }
    let mut text = collapse(spec.title.as_deref().unwrap_or(""));
    for piece in pieces {
        if piece.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&piece);
    }
    tree.text = crate::adapter::text::truncate_chars(text, super::TEXT_PROJECTION_MAX_CHARS);
    tree
}

fn increment(census: &mut BTreeMap<String, usize>, tag: &str) {
    *census.entry(tag.to_owned()).or_insert(0) += 1;
}

/// The deterministic one-line census projection (the caller bounds it
/// to [`super::TEXT_PROJECTION_MAX_CHARS`] chars).
#[must_use]
pub fn render_projection(tree: &HtmlTree) -> String {
    let tags: usize = tree.census.values().sum();
    let kinds = tree.census.len();
    let mut out = format!(
        "html: doctype={}, {} tags across {} kinds",
        tree.doctype, tags, kinds
    );
    if let Some(title) = &tree.title {
        out.push_str(&format!("; title={title}"));
    }
    if !tree.text.is_empty() {
        out.push_str(&format!("; text={}", tree.text));
    }
    out
}
