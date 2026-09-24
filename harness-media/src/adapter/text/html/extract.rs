//! The html extractor: a bounded html5gum TOKENIZER walk producing the
//! canonical [`HtmlTree`] (tag census + doctype + title + collapsed
//! character data). No DOM is built: the WHATWG tokenizer is total
//! over utf8 (error recovery is the spec), which is exactly the honest
//! bounded posture for untrusted input. Budget enforcement: deadline +
//! cumulative-size cap checked at every token; the census is capped at
//! [`MAX_TAGS`] distinct names.

use std::time::Instant;

use html5gum::{Tokenizer, Token};

use super::tree::{self, HtmlTree, MAX_TAGS};
use super::TEXT_PROJECTION_MAX_CHARS;
use crate::adapter::text::{collapse, truncate_chars};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

/// Extract the bounded census tree and text projection from html5
/// bytes under caller-provided safety budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-utf8 input (the lane is utf8
/// only - charset sniffing is out of scope, disclosed);
/// [`MediaError::ExtractTooLarge`] when the budgets or the census caps
/// trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let source = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    let started = Instant::now();
    let mut walk = Walk::default();
    for result in Tokenizer::new(source) {
        if started.elapsed() >= budgets.deadline || walk.collected > budgets.max_total_bytes {
            return Err(MediaError::ExtractTooLarge);
        }
        let token = match result {
            Ok(token) => token,
            Err(error) => match error {},
        };
        match token {
            Token::Doctype(_) => walk.tree.doctype = true,
            Token::StartTag(tag) => walk.start(
                String::from_utf8_lossy(&tag.name).to_ascii_lowercase(),
            )?,
            Token::EndTag(tag) => {
                walk.end(String::from_utf8_lossy(&tag.name).to_ascii_lowercase());
            }
            Token::String(chunk) => walk.text(&String::from_utf8_lossy(&chunk)),
            Token::Comment(_) | Token::Error(_) => {}
        }
    }
    walk.finish()
}

#[derive(Default)]
struct Walk {
    tree: HtmlTree,
    pieces: Vec<String>,
    joined_len: usize,
    title_depth: u32,
    title_taken: bool,
    title_buf: String,
    collected: u64,
}

impl Walk {
    fn start(&mut self, tag: String) -> Result<(), MediaError> {
        self.collected += tag.len() as u64 + 1;
        if !self.tree.census.contains_key(&tag) && self.tree.census.len() >= MAX_TAGS {
            return Err(MediaError::ExtractTooLarge);
        }
        *self.tree.census.entry(tag.clone()).or_insert(0) += 1;
        if tag == "title" {
            if self.title_depth == 0 && !self.title_taken {
                self.title_buf.clear();
            }
            self.title_depth += 1;
        }
        Ok(())
    }

    fn end(&mut self, tag: String) {
        if tag == "title" && self.title_depth > 0 {
            self.title_depth -= 1;
            if self.title_depth == 0 && !self.title_taken {
                self.title_taken = true;
                let collapsed = collapse(&self.title_buf);
                if !collapsed.is_empty() {
                    self.tree.title = Some(collapsed);
                }
            }
        }
    }

    fn text(&mut self, chunk: &str) {
        self.collected += chunk.len() as u64;
        if self.title_depth > 0 && !self.title_taken {
            self.title_buf.push_str(chunk);
        }
        // The census text is truncated to the projection bound anyway,
        // so collecting pieces past that prefix is pointless work.
        if self.joined_len <= TEXT_PROJECTION_MAX_CHARS {
            self.joined_len += chunk.len();
            self.pieces.push(chunk.to_owned());
        }
    }

    fn finish(mut self) -> Result<Extracted, MediaError> {
        let mut text = String::new();
        for piece in &self.pieces {
            let collapsed = collapse(piece);
            if collapsed.is_empty() {
                continue;
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&collapsed);
        }
        self.tree.text = truncate_chars(text, TEXT_PROJECTION_MAX_CHARS);
        let text_projection =
            truncate_chars(tree::render_projection(&self.tree), TEXT_PROJECTION_MAX_CHARS);
        let tree_value =
            serde_json::to_value(&self.tree).unwrap_or(serde_json::Value::Null);
        Ok(Extracted { tree: tree_value, text_projection, thumbnails: Vec::new() })
    }
}
