//! The markdown generation-spec schema: the typed block subset a model
//! may drive. Parsed strictly from [`GenerationSpec::params`] - a spec
//! that does not satisfy the contract is the typed `InvalidSpec`, never
//! a guessed default or a silent text rewrite.

use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Upper bound on blocks per document (bounded writer output).
pub const MAX_BLOCKS: usize = 10_000;

/// One markdown block in the generation subset. Tagged `"type"` - the
/// house shape (svg shapes, html blocks).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// An ATX heading, level 1-6.
    Heading {
        /// Heading level, 1-6.
        level: u8,
        /// Canonical single-line text.
        text: String,
    },
    /// A body paragraph.
    Paragraph {
        /// Canonical single-line text.
        text: String,
    },
    /// A fenced code block (backtick fence).
    CodeFence {
        /// Info-string first token (single token, no whitespace); omit
        /// for a bare fence.
        #[serde(default)]
        language: Option<String>,
        /// Literal code content (any utf8 text, verbatim).
        code: String,
    },
    /// A bullet or ordered list of plain-text items.
    List {
        /// True for an ordered (`1.`) list, false for bullets.
        ordered: bool,
        /// Item texts (>= 1), each canonical single-line.
        items: Vec<String>,
    },
    /// A GFM table (alignment not in the subset - columns unaligned).
    Table {
        /// Header cell texts (>= 1 column), each canonical single-line,
        /// non-empty.
        header: Vec<String>,
        /// Data rows; each row's width equals the header width (cells
        /// may be empty).
        rows: Vec<Vec<String>>,
    },
}

/// The markdown generation spec: the ordered block list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MdSpec {
    /// Blocks in document order.
    #[serde(default)]
    pub blocks: Vec<Block>,
}

impl MdSpec {
    /// Parse and validate a generation spec addressed to markdown.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, or a contract violation (heading level outside 1-6,
    /// non-canonical text, empty list items, ragged or empty tables,
    /// whitespace in the info token, more than [`MAX_BLOCKS`] blocks).
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Markdown {
            return Err(MediaError::InvalidSpec);
        }
        let parsed: Self =
            serde_json::from_value(spec.params.clone()).map_err(|_| MediaError::InvalidSpec)?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// The INDEPENDENT spec->tree projection (M5 provenance rule): the
    /// census tree this spec's document must extract to, computed
    /// WITHOUT running the extractor or the writer.
    #[must_use]
    pub fn canonical_tree(&self) -> serde_json::Value {
        let tree = crate::adapter::text::markdown::tree::spec_tree(self);
        serde_json::to_value(tree).unwrap_or(serde_json::Value::Null)
    }

    fn validate(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        if self.blocks.len() > MAX_BLOCKS {
            return Err(invalid());
        }
        for block in &self.blocks {
            match block {
                Block::Heading { level, text } => {
                    if !(1..=6).contains(level) || !canonical(text, true) {
                        return Err(invalid());
                    }
                }
                Block::Paragraph { text } => {
                    if !canonical(text, true) {
                        return Err(invalid());
                    }
                }
                Block::CodeFence { language, .. } => {
                    if language.as_ref().is_some_and(|token| {
                        token.is_empty() || token.contains(char::is_whitespace)
                    }) {
                        return Err(invalid());
                    }
                }
                Block::List { items, .. } => {
                    if items.is_empty() || items.iter().any(|item| !canonical(item, true)) {
                        return Err(invalid());
                    }
                }
                Block::Table { header, rows } => {
                    if header.is_empty() || header.iter().any(|cell| !canonical(cell, true)) {
                        return Err(invalid());
                    }
                    if rows.iter().any(|row| row.len() != header.len())
                        || rows.iter().flatten().any(|cell| !canonical(cell, false))
                    {
                        return Err(invalid());
                    }
                }
            }
        }
        Ok(())
    }
}

fn canonical(text: &str, require_nonempty: bool) -> bool {
    crate::adapter::text::is_canonical_text(text, require_nonempty)
}
