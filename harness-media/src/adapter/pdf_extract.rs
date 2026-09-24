//! pdf extract: lopdf DOM walk (pages, version, metadata, outline,
//! fonts-with-embeddedness, images) plus the per-page text layer under
//! the caller's ExtractBudgets. Typed failures only - no panic lane.

use std::collections::BTreeMap;
use std::time::Instant;

use lopdf::{Dictionary, Document, Object};
use serde_json::{json, Value};

use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted, TEXT_PROJECTION_MAX_CHARS};

/// Outline walk caps: pathological bookmark trees stop here (the walk is
/// bounded, like every other loop in this adapter).
const OUTLINE_NODE_CAP: usize = 1024;
const OUTLINE_DEPTH_CAP: usize = 64;

/// Metadata string bound (chars) - Info values are untrusted text.
const METADATA_MAX_CHARS: usize = 256;

/// The FontDescriptor keys that mark a font as embedded (FontFile =
/// Type1, FontFile2 = TrueType, FontFile3 = CFF/OpenType).
const FONT_FILE_KEYS: [&[u8]; 3] = [b"FontFile", b"FontFile2", b"FontFile3"];

/// The pdf extract handler: bytes + caller budgets -> canonical tree,
/// bounded text projection, no thumbnails (rasterization is not a pdf
/// lane). Fails typed: ParseFailure, EncryptedUnsupported,
/// ExtractTooLarge - never panics.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let start = Instant::now();
    let doc = Document::load_mem(bytes).map_err(|_| MediaError::ParseFailure)?;
    if doc.is_encrypted() {
        return Err(MediaError::EncryptedUnsupported);
    }

    // Budget accounting: input bytes read plus everything materialized
    // (page text is stored in the tree AND joined into the projection,
    // so it is counted twice; tree scaffolding is dominated by it).
    let mut spent: u64 = bytes.len() as u64;

    let pages = doc.get_pages();
    let mut page_entries = Vec::with_capacity(pages.len());
    let mut font_table: BTreeMap<String, Value> = BTreeMap::new();
    let mut total_images = 0_usize;
    let mut text_join = String::new();

    for (number, page_id) in &pages {
        if start.elapsed() > budgets.deadline {
            return Err(MediaError::ExtractTooLarge);
        }
        let text = doc
            .extract_text_chunks(&[*number])
            .into_iter()
            .collect::<Result<String, _>>()
            .map_err(|_| MediaError::ParseFailure)?;
        spent = spent.saturating_add((text.len() as u64).saturating_mul(2));
        if spent > budgets.max_total_bytes {
            return Err(MediaError::ExtractTooLarge);
        }

        let fonts = doc.get_page_fonts(*page_id).map_err(|_| MediaError::ParseFailure)?;
        let mut page_fonts = Vec::with_capacity(fonts.len());
        for font in fonts.values().copied() {
            let entry = font_entry(&doc, font)?;
            page_fonts.push(entry["base_font"].clone());
            let key = format!(
                "{}|{}|{}",
                entry["base_font"], entry["subtype"],
                entry.get("encoding").cloned().unwrap_or(Value::Null)
            );
            font_table.insert(key, entry);
        }
        page_fonts.dedup();
        let images = doc.get_page_images(*page_id).map(|images| images.len()).unwrap_or(0);
        total_images += images;

        text_join.push_str(&text);
        page_entries.push(json!({"fonts": page_fonts, "images": images, "text": text}));
    }

    let fonts: Vec<Value> = font_table.into_values().collect();
    let font_count = fonts.len();
    let embedded = fonts.iter().filter(|font| font["embedded"] == json!(true)).count();
    let tree = json!({
        "embedded_images": total_images,
        "fonts": fonts,
        "metadata": metadata_tree(&doc),
        "outline_depth": outline_depth(&doc),
        "page_count": page_entries.len(),
        "pages": page_entries,
        "version": doc.version,
    });

    let summary = format!(
        "pdf {} | {} pages | {} fonts ({} embedded) | {} images | outline depth {}\n",
        doc.version, tree["page_count"], font_count, embedded, total_images, tree["outline_depth"],
    );
    let mut projection = summary;
    let remaining = TEXT_PROJECTION_MAX_CHARS.saturating_sub(projection.chars().count());
    projection.push_str(&text_join.chars().take(remaining).collect::<String>());

    Ok(Extracted { tree, text_projection: projection, thumbnails: Vec::new() })
}

fn field<'a>(dict: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    dict.get(key).ok()
}

fn reference(dict: &Dictionary, key: &[u8]) -> Option<(u32, u16)> {
    field(dict, key).and_then(|object| object.as_reference().ok())
}

fn resolve<'a>(doc: &'a Document, object: &'a Object) -> Option<&'a Object> {
    doc.dereference(object).ok().map(|(_, resolved)| resolved)
}

fn subdict<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Dictionary> {
    resolve(doc, field(dict, key)?).and_then(|object| object.as_dict().ok())
}

fn bounded_string(object: &Object) -> String {
    object
        .as_str()
        .ok()
        .map(|raw| String::from_utf8_lossy(raw).chars().take(METADATA_MAX_CHARS).collect())
        .unwrap_or_default()
}

fn metadata_tree(doc: &Document) -> Value {
    let Some(info) = subdict(doc, &doc.trailer, b"Info") else {
        return json!({});
    };
    let mut metadata = serde_json::Map::new();
    for (key, name) in [
        (b"Title".as_slice(), "title"),
        (b"Author".as_slice(), "author"),
        (b"Subject".as_slice(), "subject"),
        (b"Creator".as_slice(), "creator"),
        (b"Producer".as_slice(), "producer"),
        (b"CreationDate".as_slice(), "creation_date"),
        (b"ModDate".as_slice(), "mod_date"),
    ] {
        if let Some(value) = field(info, key) {
            metadata.insert(name.into(), Value::String(bounded_string(value)));
        }
    }
    Value::Object(metadata)
}

/// One font entry for the tree: identity, subtype, embedded-ness (the
/// #1 agent-PDF defect surface), and encoding when declared.
fn font_entry(doc: &Document, font: &Dictionary) -> Result<Value, MediaError> {
    let name_of = |key: &[u8]| {
        field(font, key)
            .and_then(|object| object.as_name().ok())
            .map(|value| String::from_utf8_lossy(value).to_string())
    };
    let base_font = name_of(b"BaseFont").unwrap_or_else(|| "unknown".into());
    let subtype = name_of(b"Subtype").unwrap_or_else(|| "unknown".into());
    let encoding = name_of(b"Encoding");

    let has_font_file =
        |descriptor: &Dictionary| FONT_FILE_KEYS.iter().any(|key| descriptor.get(key).is_ok());
    let mut embedded = subdict(doc, font, b"FontDescriptor").is_some_and(has_font_file);
    if !embedded && subtype == "Type0" {
        let descendant = field(font, b"DescendantFonts")
            .and_then(|object| object.as_array().ok())
            .and_then(|array| array.first())
            .and_then(|object| resolve(doc, object))
            .and_then(|object| object.as_dict().ok());
        embedded = descendant
            .and_then(|descendant| subdict(doc, descendant, b"FontDescriptor"))
            .is_some_and(has_font_file);
    }

    let mut entry = serde_json::Map::new();
    entry.insert("base_font".into(), Value::String(base_font));
    entry.insert("subtype".into(), Value::String(subtype));
    entry.insert("embedded".into(), Value::Bool(embedded));
    if let Some(encoding) = encoding {
        entry.insert("encoding".into(), Value::String(encoding));
    }
    Ok(Value::Object(entry))
}

/// Outline (bookmark tree) depth, walked with node + depth caps. No
/// outline (the common case, and every krilla output today) is 0.
fn outline_depth(doc: &Document) -> usize {
    let first = doc
        .catalog()
        .ok()
        .and_then(|catalog| subdict(doc, catalog, b"Outlines"))
        .and_then(|outlines| reference(outlines, b"First"));
    let Some(first) = first else {
        return 0;
    };
    let mut walked = 0_usize;
    outline_walk(doc, first, 1, &mut walked)
}

fn outline_walk(doc: &Document, node: (u32, u16), depth: usize, walked: &mut usize) -> usize {
    if depth > OUTLINE_DEPTH_CAP || *walked >= OUTLINE_NODE_CAP {
        return depth.saturating_sub(1);
    }
    *walked += 1;
    let mut max = depth;
    if let Ok(dict) = doc.get_dictionary(node) {
        if let Some(child) = reference(dict, b"First") {
            max = max.max(outline_walk(doc, child, depth + 1, walked));
        }
        if let Some(next) = reference(dict, b"Next") {
            max = max.max(outline_walk(doc, next, depth, walked));
        }
    }
    max
}
