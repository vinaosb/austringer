//! The svg extractor: a bounded quick-xml walk producing the canonical
//! [`SvgTree`] plus the deterministic text projection.
//!
//! Security posture (plan M11, test-pinned): any `<!DOCTYPE>` is refused
//! typed - that is where external entities (XXE) and internal entity
//! bombs (billion laughs) live, and quick-xml never expands custom
//! entities, so the refusal is immediate. General entity references
//! resolve ONLY for the five XML predefined names and numeric character
//! references; anything else is the typed `ParseFailure`. Nothing is ever
//! fetched: `<image>` hrefs are listed, not dereferenced.

use std::collections::BTreeSet;
use std::time::Instant;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::tree::{self, SvgTree, ViewBox, ANIMATION_TAGS};
use super::TEXT_PROJECTION_MAX_CHARS;
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

const MAX_TAGS: usize = 1024;
const MAX_LIST_ENTRIES: usize = 4096;

/// Extract the bounded structural tree and text projection from svg
/// bytes, under caller-provided safety budgets (deadline or size trips
/// are the typed [`MediaError::ExtractTooLarge`]; malformed input is the
/// typed [`MediaError::ParseFailure`]). No thumbnails in v0.2 -
/// rasterization is explicitly out of scope.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-utf8/malformed XML, non-svg root,
/// DOCTYPE presence, or unresolvable entity references;
/// [`MediaError::ExtractTooLarge`] when the budgets trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let source = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    let mut reader = Reader::from_str(source);
    let started = Instant::now();
    let mut walk = Walk::default();
    loop {
        if started.elapsed() >= budgets.deadline || walk.collected > budgets.max_total_bytes {
            return Err(MediaError::ExtractTooLarge);
        }
        let event = reader.read_event().map_err(|_| MediaError::ParseFailure)?;
        match event {
            Event::Eof => break,
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) => {}
            Event::DocType(_) => return Err(MediaError::ParseFailure),
            Event::Start(element) => walk.start(&element)?,
            Event::Empty(element) => walk.start(&element)?,
            Event::End(element) => walk.end(local(element.name().as_ref())),
            Event::Text(chunk) => walk.text(&chunk),
            Event::CData(chunk) => walk.text(&chunk),
            Event::GeneralRef(reference) => walk.reference(&reference)?,
        }
    }
    if !walk.saw_root {
        return Err(MediaError::ParseFailure);
    }
    walk.finish()
}

/// Strip a namespace prefix: `svg:rect` -> `rect`.
fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

#[derive(Default)]
struct Walk {
    tree: SvgTree,
    ids: BTreeSet<String>,
    classes: BTreeSet<String>,
    rasters: BTreeSet<String>,
    texts: Vec<String>,
    current_text: String,
    text_depth: u32,
    title_depth: u32,
    desc_depth: u32,
    title_buf: String,
    desc_buf: String,
    saw_root: bool,
    collected: u64,
}

impl Walk {
    fn start(&mut self, element: &BytesStart<'_>) -> Result<(), MediaError> {
        let tag = local(element.name().as_ref()).to_owned();
        let is_root = !self.saw_root;
        if is_root {
            if tag != "svg" {
                return Err(MediaError::ParseFailure);
            }
            self.saw_root = true;
        }
        *self.tree.census.entry(tag.clone()).or_insert(0) += 1;
        if self.tree.census.len() > MAX_TAGS {
            return Err(MediaError::ExtractTooLarge);
        }
        if ANIMATION_TAGS.contains(&tag.as_str()) {
            self.tree.animation_elements += 1;
        }
        let mut view_box: Option<ViewBox> = None;
        let mut width: Option<serde_json::Value> = None;
        let mut height: Option<serde_json::Value> = None;
        for attribute in element.attributes() {
            let attribute = attribute.map_err(|_| MediaError::ParseFailure)?;
            let key = local(attribute.key.as_ref()).to_owned();
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|_| MediaError::ParseFailure)?;
            self.collected = self.collected.saturating_add(value.len() as u64 + 1);
            match key.as_str() {
                "id" => {
                    if self.ids.len() >= MAX_LIST_ENTRIES && !self.ids.contains(value.as_ref()) {
                        return Err(MediaError::ExtractTooLarge);
                    }
                    self.ids.insert(value.into_owned());
                }
                "class" => {
                    for token in value.split_whitespace() {
                        if self.classes.len() >= MAX_LIST_ENTRIES
                            && !self.classes.contains(token)
                        {
                            return Err(MediaError::ExtractTooLarge);
                        }
                        self.classes.insert(token.to_owned());
                    }
                }
                "href" if tag == "image" => {
                    if self.rasters.len() >= MAX_LIST_ENTRIES
                        && !self.rasters.contains(value.as_ref())
                    {
                        return Err(MediaError::ExtractTooLarge);
                    }
                    self.rasters.insert(value.into_owned());
                }
                "viewBox" if is_root && view_box.is_none() => {
                    view_box = tree::parse_view_box(&value);
                }
                "width" if is_root && width.is_none() => {
                    width = Some(tree::number_or_raw(&value));
                }
                "height" if is_root && height.is_none() => {
                    height = Some(tree::number_or_raw(&value));
                }
                _ => {}
            }
        }
        if is_root {
            self.tree.root.view_box = view_box;
            self.tree.root.width = width;
            self.tree.root.height = height;
        }
        match tag.as_str() {
            "text" => {
                if self.text_depth == 0 {
                    self.current_text.clear();
                }
                self.text_depth += 1;
            }
            "title" => {
                if self.title_depth == 0 {
                    self.title_buf.clear();
                }
                self.title_depth += 1;
            }
            "desc" => {
                if self.desc_depth == 0 {
                    self.desc_buf.clear();
                }
                self.desc_depth += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn end(&mut self, tag: &str) {
        match tag {
            "text" if self.text_depth > 0 => {
                self.text_depth -= 1;
                if self.text_depth == 0 {
                    let collapsed = tree::collapse(&self.current_text);
                    if !collapsed.is_empty() {
                        self.texts.push(collapsed);
                    }
                }
            }
            "title" if self.title_depth > 0 => {
                self.title_depth -= 1;
                if self.title_depth == 0 && self.tree.title.is_none() {
                    let collapsed = tree::collapse(&self.title_buf);
                    if !collapsed.is_empty() {
                        self.tree.title = Some(collapsed);
                    }
                }
            }
            "desc" if self.desc_depth > 0 => {
                self.desc_depth -= 1;
                if self.desc_depth == 0 && self.tree.desc.is_none() {
                    let collapsed = tree::collapse(&self.desc_buf);
                    if !collapsed.is_empty() {
                        self.tree.desc = Some(collapsed);
                    }
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, chunk: &str) {
        self.collected = self.collected.saturating_add(chunk.len() as u64);
        if self.text_depth > 0 {
            self.current_text.push_str(chunk);
        }
        if self.title_depth > 0 {
            self.title_buf.push_str(chunk);
        }
        if self.desc_depth > 0 {
            self.desc_buf.push_str(chunk);
        }
    }

    fn reference(&mut self, name: &str) -> Result<(), MediaError> {
        let resolved = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "apos" => '\'',
            "quot" => '"',
            _ => char_reference(name).ok_or(MediaError::ParseFailure)?,
        };
        let single = resolved.to_string();
        self.text(&single);
        Ok(())
    }

    fn finish(mut self) -> Result<Extracted, MediaError> {
        self.tree.ids = self.ids.into_iter().collect();
        self.tree.classes = self.classes.into_iter().collect();
        self.tree.embedded_rasters = self.rasters.into_iter().collect();
        self.tree.text =
            tree::truncate_chars(self.texts.join(" "), TEXT_PROJECTION_MAX_CHARS);
        if let Some(title) = self.tree.title.take() {
            self.tree.title = Some(tree::truncate_chars(title, TEXT_PROJECTION_MAX_CHARS));
        }
        if let Some(desc) = self.tree.desc.take() {
            self.tree.desc = Some(tree::truncate_chars(desc, TEXT_PROJECTION_MAX_CHARS));
        }
        let text_projection =
            tree::truncate_chars(tree::render_projection(&self.tree), TEXT_PROJECTION_MAX_CHARS);
        let tree_value = match serde_json::to_value(&self.tree) {
            Ok(value) => value,
            Err(_) => serde_json::Value::Null,
        };
        Ok(Extracted { tree: tree_value, text_projection, thumbnails: Vec::new() })
    }
}

/// Numeric character reference: `#65` / `#x41`.
fn char_reference(name: &str) -> Option<char> {
    let digits = name.strip_prefix("#x").or_else(|| name.strip_prefix('#'))?;
    let radix = if name.starts_with("#x") { 16 } else { 10 };
    u32::from_str_radix(digits, radix).ok().and_then(char::from_u32)
}
