//! The canonical markdown census tree: the bounded structural summary
//! BOTH the extractor (walking the comrak AST) and the independent
//! spec->tree projection produce. Same shape, two builders, separately
//! tested - the M5 provenance rule's markdown instance.

use std::collections::BTreeMap;

use serde::Serialize;

use super::spec::{Block, MdSpec};

/// Upper bound on headings one document may carry (bounded census).
pub const MAX_HEADINGS: usize = 4096;

/// One heading entry: level + collapsed text, document order.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HeadingEntry {
    /// Heading level, 1-6.
    pub level: u8,
    /// Whitespace-collapsed heading text.
    pub text: String,
}

/// The bounded markdown census. `code_blocks` counts fenced AND
/// indented blocks: comrak's canonical formatter renders a bare fence
/// (empty info string) as an indented block - the canonical CommonMark
/// spelling - so the census counts code blocks and both spellings land
/// identically (dialect pinning through the shared parser).
#[derive(Debug, Clone, Default, Serialize)]
pub struct MdTree {
    /// Count of code blocks (fenced or indented).
    pub code_blocks: usize,
    /// Occurrences per info-string first token (`""` for bare fences
    /// and indented blocks).
    pub code_languages: BTreeMap<String, usize>,
    /// Headings in document order.
    pub headings: Vec<HeadingEntry>,
    /// Count of list nodes (bullet or ordered).
    pub lists: usize,
    /// Count of paragraph nodes anywhere in the tree (a tight list
    /// item's inner paragraph included - the shape the canonical writer
    /// emits is the shape the projection predicts).
    pub paragraphs: usize,
    /// Count of GFM table nodes.
    pub tables: usize,
}

/// The INDEPENDENT spec->tree projection: computed without the writer
/// and without the extractor.
#[must_use]
pub fn spec_tree(spec: &MdSpec) -> MdTree {
    let mut tree = MdTree::default();
    for block in &spec.blocks {
        match block {
            Block::Heading { level, text } => {
                tree.headings.push(HeadingEntry {
                    level: *level,
                    text: super::super::collapse(text),
                });
            }
            Block::Paragraph { .. } => {
                tree.paragraphs += 1;
            }
            Block::CodeFence { language, .. } => {
                tree.code_blocks += 1;
                increment(&mut tree.code_languages, language.as_deref().unwrap_or(""));
            }
            Block::List { items, .. } => {
                tree.lists += 1;
                tree.paragraphs += items.len();
            }
            Block::Table { .. } => {
                tree.tables += 1;
            }
        }
    }
    tree
}

fn increment(map: &mut BTreeMap<String, usize>, key: &str) {
    *map.entry(key.to_owned()).or_insert(0) += 1;
}

/// The deterministic outline projection of a tree (the caller bounds it
/// to [`super::TEXT_PROJECTION_MAX_CHARS`] chars).
#[must_use]
pub fn render_projection(tree: &MdTree) -> String {
    let outline: Vec<String> = tree
        .headings
        .iter()
        .map(|heading| format!("{}{}", "#".repeat(heading.level.min(6) as usize), heading.text))
        .collect();
    let languages: Vec<String> = tree
        .code_languages
        .iter()
        .map(|(language, count)| format!("{language}={count}"))
        .collect();
    let mut out = format!(
        "markdown: {} headings, {} paragraphs, {} lists, {} code_blocks, {} tables",
        tree.headings.len(),
        tree.paragraphs,
        tree.lists,
        tree.code_blocks,
        tree.tables
    );
    if !languages.is_empty() {
        out.push_str(&format!("; languages={}", languages.join(",")));
    }
    if !outline.is_empty() {
        out.push_str(&format!("; outline: {}", outline.join(" > ")));
    }
    out
}
