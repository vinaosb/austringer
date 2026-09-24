//! The html generation-spec schema: the typed block subset a model may
//! drive. Strict parse from [`GenerationSpec::params`] - contract
//! violations are the typed `InvalidSpec`, never a silent rewrite.

use serde::{Deserialize, Serialize};

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Upper bound on blocks per document (bounded writer output).
pub const MAX_BLOCKS: usize = 10_000;

/// One html body block in the generation subset. Tagged `"type"` - the
/// house shape (svg shapes, markdown blocks).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HtmlBlock {
    /// An `<h1>`-`<h6>` heading.
    Heading {
        /// Heading level, 1-6.
        level: u8,
        /// Canonical single-line text.
        text: String,
    },
    /// A `<p>` paragraph.
    Paragraph {
        /// Canonical single-line text.
        text: String,
    },
    /// A `<ul>`/`<ol>` list of plain-text items.
    List {
        /// True for `<ol>`, false for `<ul>`.
        ordered: bool,
        /// Item texts (>= 1), each canonical single-line.
        items: Vec<String>,
    },
    /// A `<pre><code>` block.
    Code {
        /// Code content (any utf8 text, verbatim, maud-escaped).
        text: String,
    },
    /// A `<table>` with a `<thead>` header row and `<tbody>` data rows.
    Table {
        /// Header cell texts (>= 1), each canonical single-line,
        /// non-empty.
        header: Vec<String>,
        /// Data rows; each row's width equals the header width (cells
        /// may be empty).
        rows: Vec<Vec<String>>,
    },
}

/// The html generation spec: optional `<title>` plus the ordered block
/// list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlSpec {
    /// `<title>` content (self-contained documents declare one).
    #[serde(default)]
    pub title: Option<String>,
    /// Body blocks in document order.
    #[serde(default)]
    pub blocks: Vec<HtmlBlock>,
}

impl HtmlSpec {
    /// Parse and validate a generation spec addressed to html.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, or a contract violation (heading level outside 1-6,
    /// non-canonical text, empty list items, ragged or empty tables,
    /// more than [`MAX_BLOCKS`] blocks).
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Html {
            return Err(MediaError::InvalidSpec);
        }
        let parsed: Self =
            serde_json::from_value(spec.params.clone()).map_err(|_| MediaError::InvalidSpec)?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// The INDEPENDENT spec->tree projection (M5 provenance rule):
    /// computed without the writer and without the extractor.
    #[must_use]
    pub fn canonical_tree(&self) -> serde_json::Value {
        let tree = crate::adapter::text::html::tree::spec_tree(self);
        serde_json::to_value(tree).unwrap_or(serde_json::Value::Null)
    }

    fn validate(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        if self.blocks.len() > MAX_BLOCKS {
            return Err(invalid());
        }
        if self.title.as_ref().is_some_and(|title| !canonical(title, true)) {
            return Err(invalid());
        }
        for block in &self.blocks {
            match block {
                HtmlBlock::Heading { level, text } => {
                    if !(1..=6).contains(level) || !canonical(text, true) {
                        return Err(invalid());
                    }
                }
                HtmlBlock::Paragraph { text } => {
                    if !canonical(text, true) {
                        return Err(invalid());
                    }
                }
                // Code content is verbatim (newlines preserved by
                // <pre>); both census sides collapse it identically.
                HtmlBlock::Code { .. } => {}
                HtmlBlock::List { items, .. } => {
                    if items.is_empty() || items.iter().any(|item| !canonical(item, true)) {
                        return Err(invalid());
                    }
                }
                HtmlBlock::Table { header, rows } => {
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
