//! The markdown extractor: a bounded walk over the comrak AST producing
//! the canonical [`MdTree`] plus the bounded outline projection.
//!
//! CommonMark is total over utf8 text - the only typed `ParseFailure`
//! is non-utf8 bytes. Budget enforcement: input-size pre-check bounds
//! the one-shot parse; the walk checks the wall-clock deadline and the
//! cumulative-size cap at every node; the heading list is capped at
//! [`MAX_HEADINGS`] entries.

use std::time::Instant;

use comrak::nodes::NodeValue;
use comrak::{Arena, parse_document};

use super::TEXT_PROJECTION_MAX_CHARS;
use super::tree::{self, HeadingEntry, MAX_HEADINGS, MdTree};
use crate::adapter::text::markdown::generate::gfm_options;
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

/// Extract the bounded census tree and outline projection from markdown
/// bytes under caller-provided safety budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-utf8 input (the only malformed
/// markdown there is); [`MediaError::ExtractTooLarge`] when the budgets
/// or the heading cap trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let source = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    if source.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let started = Instant::now();
    let arena = Arena::new();
    let root = parse_document(&arena, source, &gfm_options());
    let mut walk = Walk::default();
    for node in root.descendants() {
        if started.elapsed() >= budgets.deadline || walk.collected > budgets.max_total_bytes {
            return Err(MediaError::ExtractTooLarge);
        }
        let data = node.data();
        match &data.value {
            NodeValue::Heading(heading) => {
                if walk.tree.headings.len() >= MAX_HEADINGS {
                    return Err(MediaError::ExtractTooLarge);
                }
                let text = tree_collapse(&node.collect_text());
                walk.collected += text.len() as u64;
                walk.tree.headings.push(HeadingEntry {
                    level: heading.level,
                    text,
                });
            }
            NodeValue::Paragraph => {
                walk.tree.paragraphs += 1;
                walk.collected += 1;
            }
            NodeValue::CodeBlock(block) => {
                walk.tree.code_blocks += 1;
                let language = block
                    .info
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_owned();
                walk.collected += language.len() as u64 + 1;
                *walk.tree.code_languages.entry(language).or_insert(0) += 1;
            }
            NodeValue::List(_) => {
                walk.tree.lists += 1;
                walk.collected += 1;
            }
            NodeValue::Table(_) => {
                walk.tree.tables += 1;
                walk.collected += 1;
            }
            _ => {}
        }
    }
    finish(walk.tree)
}

fn tree_collapse(raw: &str) -> String {
    crate::adapter::text::collapse(raw)
}

fn finish(tree: MdTree) -> Result<Extracted, MediaError> {
    let text_projection = crate::adapter::text::truncate_chars(
        tree::render_projection(&tree),
        TEXT_PROJECTION_MAX_CHARS,
    );
    let tree_value = serde_json::to_value(&tree).unwrap_or(serde_json::Value::Null);
    Ok(Extracted {
        tree: tree_value,
        text_projection,
        thumbnails: Vec::new(),
    })
}

#[derive(Default)]
struct Walk {
    tree: MdTree,
    collected: u64,
}
