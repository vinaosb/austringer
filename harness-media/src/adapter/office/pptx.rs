//! The pptx extractor: slide count in presentation order, per-slide
//! shape census (sp/pic/graphicFrame) and text runs. Slide-master and
//! layout inheritance is deliberately NOT interpreted (the digest's
//! "where naive parsers fail" note) - the census is per slide part
//! only. Projection: slide/shape/text summary.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};

use super::TEXT_PROJECTION_MAX_CHARS;
use super::parts::{self, Ledger, collapse, truncate_chars};
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted};

const MAX_SLIDES: usize = 512;
const MAX_RUNS: usize = 256;
const MAX_SHAPES: usize = 2048;

/// Extract the pptx structural tree and bounded text projection under
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
    let presentation = package.read_xml("ppt/presentation.xml")?;
    ledger.charge(presentation.len())?;
    let rels_xml = package.read_xml_optional("ppt/_rels/presentation.xml.rels")?;
    let macros = u8::from(package.has_part("ppt/vbaProject.bin"));

    let rels = match rels_xml {
        Some(xml) => {
            ledger.charge(xml.len())?;
            parts::parse_rels(&xml, &mut ledger)?
        }
        None => Vec::new(),
    };

    let mut slides = Vec::new();
    for rid in slide_ids(&presentation, &mut ledger)? {
        let target = rels
            .iter()
            .find(|rel| rel.id == rid && !rel.external)
            .map(|rel| rel.target.clone())
            .ok_or(MediaError::ParseFailure)?;
        if slides.len() >= MAX_SLIDES {
            return Err(MediaError::ExtractTooLarge);
        }
        let part = package.read_xml(&format!("ppt/{target}"))?;
        ledger.charge(part.len())?;
        slides.push(slide_tree(&part, &mut ledger)?);
    }
    let count = slides.len();
    let mut projection = format!("pptx: {count} slides");
    for (index, slide) in slides.iter().enumerate() {
        let shapes = slide["shapes"].as_object().map_or(0, |census| {
            census.values().filter_map(Value::as_u64).sum::<u64>()
        });
        projection.push_str(&format!("\nslide {}: {shapes} shapes", index + 1));
        let runs = slide["text_runs"].as_array().into_iter().flatten();
        let texts = runs
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("; ");
        if !texts.is_empty() {
            projection.push_str(&format!("; text: {texts}"));
        }
    }
    if macros > 0 {
        projection.push_str(&format!("\nmacros present: {macros}, not executed"));
    }
    let text_projection = truncate_chars(projection, TEXT_PROJECTION_MAX_CHARS);
    let tree = json!({
        "macros": {"present": macros},
        "slide_count": count,
        "slides": slides,
    });
    Ok(Extracted {
        tree,
        text_projection,
        thumbnails: Vec::new(),
    })
}

/// The presentation's slide relationship ids in document order.
fn slide_ids(presentation: &str, ledger: &mut Ledger) -> Result<Vec<String>, MediaError> {
    let mut reader = Reader::from_str(presentation);
    let mut ids = Vec::new();
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            Event::Start(element) | Event::Empty(element)
                if element.name().local_name().as_ref() == "sldId" =>
            {
                if ids.len() >= MAX_SLIDES {
                    return Err(MediaError::ExtractTooLarge);
                }
                // r:id (relationship) - matched by full prefixed key;
                // the unprefixed `id` is the numeric slide id.
                ids.push(
                    parts::prefixed_attribute(&element, "r:id").ok_or(MediaError::ParseFailure)?,
                );
            }
            _ => {}
        }
    }
    Ok(ids)
}

/// One slide part -> its canonical tree object (shape census + text
/// runs).
fn slide_tree(part: &str, ledger: &mut Ledger) -> Result<Value, MediaError> {
    let mut reader = Reader::from_str(part);
    let mut sp = 0_usize;
    let mut pic = 0_usize;
    let mut graphic_frame = 0_usize;
    let mut runs: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_text = false;
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            // Census elements count both Start and Empty forms; text
            // runs open only on Start (Empty has no matching End).
            Event::Start(element) => match element.name().local_name().as_ref() {
                "t" => {
                    in_text = true;
                    current.clear();
                }
                "sp" => counted(&mut sp)?,
                "pic" => counted(&mut pic)?,
                "graphicFrame" => counted(&mut graphic_frame)?,
                _ => {}
            },
            Event::Empty(element) => match element.name().local_name().as_ref() {
                "sp" => counted(&mut sp)?,
                "pic" => counted(&mut pic)?,
                "graphicFrame" => counted(&mut graphic_frame)?,
                _ => {}
            },
            Event::Text(chunk) if in_text => current.push_str(&chunk),
            Event::GeneralRef(reference) if in_text => {
                let resolved = parts::reference_char(&reference)
                    .ok_or(MediaError::ParseFailure)?
                    .to_string();
                current.push_str(&resolved);
            }
            Event::End(element) if element.name().local_name().as_ref() == "t" && in_text => {
                in_text = false;
                let run = collapse(&current);
                if !run.is_empty() {
                    if runs.len() >= MAX_RUNS {
                        return Err(MediaError::ExtractTooLarge);
                    }
                    runs.push(run);
                }
            }
            _ => {}
        }
    }
    Ok(json!({
        "shapes": {"graphic_frame": graphic_frame, "pic": pic, "sp": sp},
        "text_runs": runs,
    }))
}

fn counted(total: &mut usize) -> Result<(), MediaError> {
    *total += 1;
    if *total > MAX_SHAPES {
        return Err(MediaError::ExtractTooLarge);
    }
    Ok(())
}
