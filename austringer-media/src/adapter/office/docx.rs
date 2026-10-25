//! The docx extractor: a bounded quick-xml walk over `word/document.xml`
//! (paragraphs with styles/headings, tables, lists, embedded image
//! refs) plus the styles.xml defined-styles census, the document rels
//! (images resolved, external rels listed), and the vbaProject.bin
//! macro disclosure. Projection: bounded markdown-ish text.

use std::collections::BTreeSet;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};

use super::TEXT_PROJECTION_MAX_CHARS;
use super::parts::{self, Ledger, Rel, collapse, truncate_chars};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

const MAX_PARAGRAPHS: usize = 2048;
const MAX_TABLES: usize = 256;
const MAX_STYLES: usize = 1024;
const MAX_REFS: usize = 1024;
const MAX_PARAGRAPH_CHARS: usize = 512;

/// Extract the docx structural tree and bounded text projection under
/// caller-provided safety budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on a non-zip package, a missing or
/// malformed required part, or an unresolvable entity reference;
/// [`MediaError::ExtractTooLarge`] when the budgets or structural caps
/// trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let mut ledger = Ledger::new(bytes.len(), budgets)?;
    let mut package = parts::Package::open(bytes)?;
    let document = package.read_xml("word/document.xml")?;
    ledger.charge(document.len())?;
    let styles = package.read_xml_optional("word/styles.xml")?;
    let rels_xml = package.read_xml_optional("word/_rels/document.xml.rels")?;
    let macros = u8::from(package.has_part("word/vbaProject.bin"));

    let mut walk = Walk::default();
    walk_document(&document, &mut ledger, &mut walk)?;
    let styles_defined = match styles {
        Some(xml) => {
            ledger.charge(xml.len())?;
            style_census(&xml, &mut ledger)?
        }
        None => BTreeSet::new(),
    };
    let rels = match rels_xml {
        Some(xml) => {
            ledger.charge(xml.len())?;
            parts::parse_rels(&xml, &mut ledger)?
        }
        None => Vec::new(),
    };
    let (images, external) = resolve_refs(&walk.embeds, &rels);
    finish(walk, &styles_defined, images, external, macros)
}

/// One pass over the body: paragraphs (outside tables), table census,
/// and embedded image reference ids.
#[derive(Default)]
struct Walk {
    paragraphs: Vec<Value>,
    open: bool,
    style: Option<String>,
    list: bool,
    text: String,
    table_depth: u32,
    tables: Vec<(usize, usize)>,
    rows: usize,
    cols: usize,
    first_row: bool,
    embeds: BTreeSet<String>,
}

fn walk_document(document: &str, ledger: &mut Ledger, walk: &mut Walk) -> Result<(), MediaError> {
    let mut reader = Reader::from_str(document);
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) | Event::DocType(_) => {}
            Event::Start(element) => start(walk, &element, true)?,
            Event::Empty(element) => start(walk, &element, false)?,
            Event::End(element) => end(walk, element.name().local_name().as_ref())?,
            Event::Text(chunk) => {
                if walk.open && walk.table_depth == 0 {
                    walk.text.push_str(&chunk);
                }
            }
            Event::CData(chunk) => {
                if walk.open && walk.table_depth == 0 {
                    walk.text.push_str(&chunk);
                }
            }
            Event::GeneralRef(reference) => {
                if walk.open && walk.table_depth == 0 {
                    let resolved = parts::reference_char(&reference)
                        .ok_or(MediaError::ParseFailure)?
                        .to_string();
                    walk.text.push_str(&resolved);
                }
            }
        }
    }
    Ok(())
}

/// Reacts to one opening element. Stateful opens (`p`, `tbl`) fire
/// only on Start events - an Empty event has no matching End, so
/// opening there would strand the walker state.
fn start(walk: &mut Walk, element: &BytesStart<'_>, opens: bool) -> Result<(), MediaError> {
    let local_name = element.name().local_name();
    let tag: &str = local_name.as_ref();
    match tag {
        "p" if opens && walk.table_depth == 0 => {
            walk.open = true;
            walk.style = None;
            walk.list = false;
            walk.text.clear();
        }
        "tbl" if opens => {
            walk.table_depth += 1;
            walk.rows = 0;
            walk.cols = 0;
            walk.first_row = true;
        }
        "tr" if walk.table_depth > 0 => {
            walk.rows += 1;
        }
        "tc" if walk.table_depth > 0 && walk.first_row => {
            walk.cols += 1;
        }
        "pStyle" if walk.open => {
            walk.style = parts::attribute(element, "val");
        }
        "numPr" if walk.open => {
            walk.list = true;
        }
        "blip" => {
            if let Some(embed) = parts::prefixed_attribute(element, "r:embed") {
                if walk.embeds.len() >= MAX_REFS && !walk.embeds.contains(&embed) {
                    return Err(MediaError::ExtractTooLarge);
                }
                walk.embeds.insert(embed);
            }
        }
        _ => {}
    }
    Ok(())
}

fn end(walk: &mut Walk, tag: &str) -> Result<(), MediaError> {
    match tag {
        "p" if walk.open => {
            walk.open = false;
            let text = truncate_chars(collapse(&walk.text), MAX_PARAGRAPH_CHARS);
            if !text.is_empty() {
                if walk.paragraphs.len() >= MAX_PARAGRAPHS {
                    return Err(MediaError::ExtractTooLarge);
                }
                walk.paragraphs.push(json!({
                    "list": walk.list,
                    "style": walk.style,
                    "text": text,
                }));
            }
        }
        "tr" if walk.table_depth > 0 => {
            walk.first_row = false;
        }
        "tbl" if walk.table_depth > 0 => {
            walk.table_depth -= 1;
            if walk.tables.len() >= MAX_TABLES {
                return Err(MediaError::ExtractTooLarge);
            }
            walk.tables.push((walk.rows, walk.cols));
        }
        _ => {}
    }
    Ok(())
}

/// The defined-styles census: every `w:style` styleId in styles.xml
/// (the M19 round-trip guard - a writer whose styles.xml is broken
/// fails the diff even when paragraph style names match).
fn style_census(xml: &str, ledger: &mut Ledger) -> Result<BTreeSet<String>, MediaError> {
    let mut reader = Reader::from_str(xml);
    let mut styles = BTreeSet::new();
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            Event::Start(element) | Event::Empty(element)
                if element.name().local_name().as_ref() == "style" =>
            {
                if let Some(style_id) = parts::attribute(&element, "styleId") {
                    if styles.len() >= MAX_STYLES && !styles.contains(&style_id) {
                        return Err(MediaError::ExtractTooLarge);
                    }
                    styles.insert(style_id);
                }
            }
            _ => {}
        }
    }
    Ok(styles)
}

/// Embed ids -> image part targets (unresolved ids stay verbatim,
/// honestly); external rel targets listed.
fn resolve_refs(embeds: &BTreeSet<String>, rels: &[Rel]) -> (Vec<String>, Vec<String>) {
    let images = embeds
        .iter()
        .map(|embed| {
            rels.iter()
                .find(|rel| rel.id == *embed && !rel.external)
                .map_or_else(|| embed.clone(), |rel| rel.target.clone())
        })
        .collect();
    let external = rels
        .iter()
        .filter(|rel| rel.external)
        .map(|rel| rel.target.clone())
        .collect();
    (images, external)
}

fn finish(
    walk: Walk,
    styles: &BTreeSet<String>,
    images: Vec<String>,
    external: Vec<String>,
    macros: u8,
) -> Result<Extracted, MediaError> {
    let tables: Vec<Value> = walk
        .tables
        .iter()
        .map(|(rows, cols)| json!({"cols": cols, "rows": rows}))
        .collect();
    let tree = json!({
        "external_rels": external,
        "images": images,
        "macros": {"present": macros},
        "paragraphs": walk.paragraphs,
        "styles_defined": styles.iter().map(String::as_str).collect::<Vec<_>>(),
        "tables": tables,
    });
    let mut projection = format!(
        "docx: {} paragraphs, {} tables",
        tree["paragraphs"].as_array().map_or(0, Vec::len),
        tree["tables"].as_array().map_or(0, Vec::len),
    );
    for paragraph in tree["paragraphs"].as_array().into_iter().flatten() {
        let text = paragraph["text"].as_str().unwrap_or_default();
        let prefix = match paragraph["style"].as_str() {
            Some("Heading1") => "# ",
            Some("Heading2") => "## ",
            _ if paragraph["list"].as_bool().unwrap_or(false) => "- ",
            _ => "",
        };
        projection.push('\n');
        projection.push_str(prefix);
        projection.push_str(text);
    }
    for table in tree["tables"].as_array().into_iter().flatten() {
        projection.push_str(&format!(
            "\ntable: {} rows x {} cols",
            table["rows"], table["cols"]
        ));
    }
    if macros > 0 {
        projection.push_str(&format!("\nmacros present: {macros}, not executed"));
    }
    let text_projection = truncate_chars(projection, TEXT_PROJECTION_MAX_CHARS);
    Ok(Extracted {
        tree,
        text_projection,
        thumbnails: Vec::new(),
    })
}
