//! The xlsx extractor: workbook sheet list (in order), shared strings
//! resolved, the styles.xml defined-styles census, and per-sheet
//! worksheet trees (delegated to [`super::xlsx_sheet`]). Projection:
//! per-sheet schema + stats.

use std::collections::BTreeSet;

use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Value};

use super::parts::{self, Ledger, truncate_chars};
use super::xlsx_sheet::sheet_tree;
use super::TEXT_PROJECTION_MAX_CHARS;
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

const MAX_SHEETS: usize = 64;
const MAX_SHARED: usize = 8192;
const MAX_STYLES: usize = 1024;
const MAX_TEXT_CHARS: usize = 256;
const HEAD_SAMPLE: usize = 8;

/// Extract the xlsx structural tree and bounded text projection under
/// caller-provided safety budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on a non-zip package, a missing or
/// malformed required part, or a shared-string index out of range;
/// [`MediaError::ExtractTooLarge`] when the budgets or structural caps
/// trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let mut ledger = Ledger::new(bytes.len(), budgets)?;
    let mut package = parts::Package::open(bytes)?;
    let workbook = package.read_xml("xl/workbook.xml")?;
    ledger.charge(workbook.len())?;
    let rels_xml = package.read_xml_optional("xl/_rels/workbook.xml.rels")?;
    let shared_xml = package.read_xml_optional("xl/sharedStrings.xml")?;
    let styles_xml = package.read_xml_optional("xl/styles.xml")?;
    let macros = u8::from(package.has_part("xl/vbaProject.bin"));

    let shared = match shared_xml {
        Some(xml) => {
            ledger.charge(xml.len())?;
            shared_strings(&xml, &mut ledger)?
        }
        None => Vec::new(),
    };
    let styles = match styles_xml {
        Some(xml) => {
            ledger.charge(xml.len())?;
            named_styles(&xml, &mut ledger)?
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

    let mut sheets = Vec::new();
    for (name, rid) in sheet_list(&workbook, &mut ledger)? {
        let target = rels
            .iter()
            .find(|rel| rel.id == rid && !rel.external)
            .map(|rel| rel.target.clone())
            .ok_or(MediaError::ParseFailure)?;
        if sheets.len() >= MAX_SHEETS {
            return Err(MediaError::ExtractTooLarge);
        }
        let part = package.read_xml(&format!("xl/{target}"))?;
        ledger.charge(part.len())?;
        sheets.push(sheet_tree(&name, &part, &shared, &mut ledger)?);
    }
    finish(sheets, &styles, macros)
}

/// The workbook sheet order: (name, r:id) pairs.
fn sheet_list(workbook: &str, ledger: &mut Ledger) -> Result<Vec<(String, String)>, MediaError> {
    let mut reader = Reader::from_str(workbook);
    let mut sheets = Vec::new();
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            Event::Start(element) | Event::Empty(element)
                if element.name().local_name().as_ref() == "sheet" =>
            {
                if sheets.len() >= MAX_SHEETS {
                    return Err(MediaError::ExtractTooLarge);
                }
                let rid = parts::prefixed_attribute(&element, "r:id")
                    .ok_or(MediaError::ParseFailure)?;
                sheets.push((
                    parts::attribute(&element, "name").unwrap_or_default(),
                    rid,
                ));
            }
            _ => {}
        }
    }
    Ok(sheets)
}

/// sharedStrings.xml entries in index order (rich-text `<si>` runs are
/// concatenated - the shared-strings vs inline fidelity note).
fn shared_strings(xml: &str, ledger: &mut Ledger) -> Result<Vec<String>, MediaError> {
    let mut reader = Reader::from_str(xml);
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_item = false;
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            // Stateful elements (si) open ONLY on Start: an Empty event
            // has no matching End, so opening there would strand state.
            Event::Start(element)
                if element.name().local_name().as_ref() == "si" =>
            {
                in_item = true;
                current.clear();
            }
            Event::Text(chunk) if in_item => current.push_str(&chunk),
            Event::GeneralRef(reference) if in_item => {
                let resolved = parts::reference_char(&reference)
                    .ok_or(MediaError::ParseFailure)?
                    .to_string();
                current.push_str(&resolved);
            }
            Event::End(element)
                if element.name().local_name().as_ref() == "si" =>
            {
                in_item = false;
                if strings.len() >= MAX_SHARED {
                    return Err(MediaError::ExtractTooLarge);
                }
                strings.push(truncate_chars(current.clone(), MAX_TEXT_CHARS));
            }
            _ => {}
        }
    }
    Ok(strings)
}

/// The defined-styles census: named `cellStyle` entries in styles.xml.
fn named_styles(xml: &str, ledger: &mut Ledger) -> Result<BTreeSet<String>, MediaError> {
    let mut reader = Reader::from_str(xml);
    let mut styles = BTreeSet::new();
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            Event::Start(element) | Event::Empty(element)
                if element.name().local_name().as_ref() == "cellStyle" =>
            {
                if let Some(name) = parts::attribute(&element, "name") {
                    if styles.len() >= MAX_STYLES && !styles.contains(&name) {
                        return Err(MediaError::ExtractTooLarge);
                    }
                    styles.insert(name);
                }
            }
            _ => {}
        }
    }
    Ok(styles)
}

fn finish(sheets: Vec<Value>, styles: &BTreeSet<String>, macros: u8) -> Result<Extracted, MediaError> {
    let count = sheets.len();
    let mut projection = format!("xlsx: {count} sheets");
    for sheet in &sheets {
        let head = sheet["head_cells"]
            .as_array()
            .into_iter()
            .flatten()
            .take(HEAD_SAMPLE)
            .map(|cell| cell["value"].as_str().unwrap_or_default())
            .collect::<Vec<_>>()
            .join(", ");
        projection.push_str(&format!(
            "\n{}: {} rows x {} cols; head: {head}",
            sheet["name"].as_str().unwrap_or_default(),
            sheet["dimension"]["rows"],
            sheet["dimension"]["cols"],
        ));
    }
    if macros > 0 {
        projection.push_str(&format!("\nmacros present: {macros}, not executed"));
    }
    let text_projection = truncate_chars(projection, TEXT_PROJECTION_MAX_CHARS);
    let tree = json!({
        "macros": {"present": macros},
        "sheets": sheets,
        "styles_defined": styles.iter().map(String::as_str).collect::<Vec<_>>(),
    });
    Ok(Extracted { tree, text_projection, thumbnails: Vec::new() })
}
