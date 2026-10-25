//! The xlsx worksheet walker: bounded used-range head cells with the
//! shared table resolved, formulas listed AS TEXT with an `=` prefix
//! (never evaluated - stored `<v>` values are data), and the dimension
//! from the part's declared range.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};

use super::parts::{self, Ledger, truncate_chars};
use crate::error::MediaError;

const MAX_CELLS: usize = 4096;
const MAX_FORMULAS: usize = 1024;
const MAX_CELL_CHARS: usize = 256;

/// What one `<c>` element is currently collecting.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Capture {
    Formula,
    Value,
    Inline,
}

/// One cell under construction.
struct Cell {
    reference: String,
    kind_attr: Option<String>,
    capture: Option<Capture>,
    formula: String,
    value: String,
    inline: String,
}

/// One worksheet -> its canonical tree object.
pub(super) fn sheet_tree(
    name: &str,
    part: &str,
    shared: &[String],
    ledger: &mut Ledger,
) -> Result<Value, MediaError> {
    let mut reader = Reader::from_str(part);
    let mut dimension: Option<(usize, usize)> = None;
    let mut cells: Vec<Value> = Vec::new();
    let mut formulas: Vec<String> = Vec::new();
    let mut cell = Cell {
        reference: String::new(),
        kind_attr: None,
        capture: None,
        formula: String::new(),
        value: String::new(),
        inline: String::new(),
    };
    let mut in_cell = false;
    let mut in_inline = false;
    loop {
        match parts::next_event(&mut reader, ledger)? {
            Event::Eof => break,
            // Stateful elements (c/f/v/is/t) open ONLY on Start (Empty
            // has no End); the stateless dimension attr reads both.
            Event::Start(element) => match element.name().local_name().as_ref() {
                "dimension" => {
                    if let Some(reference) = parts::attribute(&element, "ref") {
                        dimension = dimension_of(&reference);
                    }
                }
                "c" => {
                    cell.reference = parts::attribute(&element, "r").unwrap_or_default();
                    cell.kind_attr = parts::attribute(&element, "t");
                    cell.formula.clear();
                    cell.value.clear();
                    cell.inline.clear();
                    in_cell = true;
                    in_inline = false;
                }
                "f" if in_cell => cell.capture = Some(Capture::Formula),
                "v" if in_cell => cell.capture = Some(Capture::Value),
                "is" if in_cell => in_inline = true,
                "t" if in_inline => cell.capture = Some(Capture::Inline),
                _ => {}
            },
            Event::Empty(element) if element.name().local_name().as_ref() == "dimension" => {
                if let Some(reference) = parts::attribute(&element, "ref") {
                    dimension = dimension_of(&reference);
                }
            }
            Event::Text(chunk) => {
                if let Some(target) = capture_target(&mut cell, in_cell) {
                    target.push_str(&chunk);
                }
            }
            Event::GeneralRef(reference) => {
                let resolved = parts::reference_char(&reference)
                    .ok_or(MediaError::ParseFailure)?
                    .to_string();
                if let Some(target) = capture_target(&mut cell, in_cell) {
                    target.push_str(&resolved);
                }
            }
            Event::End(element) => match element.name().local_name().as_ref() {
                "c" if in_cell => {
                    in_cell = false;
                    if let Some(cell_value) = finalize(&cell, shared, &mut formulas)? {
                        if cells.len() >= MAX_CELLS {
                            return Err(MediaError::ExtractTooLarge);
                        }
                        cells.push(cell_value);
                    }
                }
                "is" => in_inline = false,
                "f" | "v" | "t" => cell.capture = None,
                _ => {}
            },
            _ => {}
        }
    }
    let (rows, cols) = dimension.unwrap_or((0, 0));
    Ok(json!({
        "dimension": {"cols": cols, "rows": rows},
        "formulas": formulas,
        "head_cells": cells,
        "name": name,
    }))
}

/// The mutable buffer the current sub-element (`f`/`v`/inline `t`)
/// appends into; `None` when no capture is open.
fn capture_target(cell: &mut Cell, in_cell: bool) -> Option<&mut String> {
    if !in_cell {
        return None;
    }
    match cell.capture {
        Some(Capture::Formula) => Some(&mut cell.formula),
        Some(Capture::Value) => Some(&mut cell.value),
        Some(Capture::Inline) => Some(&mut cell.inline),
        None => None,
    }
}

/// Emits one cell's canonical form (`None` for a valueless cell).
fn finalize(
    cell: &Cell,
    shared: &[String],
    formulas: &mut Vec<String>,
) -> Result<Option<Value>, MediaError> {
    if !cell.formula.is_empty() {
        if formulas.len() >= MAX_FORMULAS {
            return Err(MediaError::ExtractTooLarge);
        }
        formulas.push(format!("{} ={}", cell.reference, cell.formula));
        return Ok(Some(json!({
            "kind": "formula",
            "ref": cell.reference,
            "value": truncate_chars(format!("={}", cell.formula), MAX_CELL_CHARS),
        })));
    }
    let (kind, value): (&str, String) = match cell.kind_attr.as_deref() {
        Some("s") => {
            let index = cell
                .value
                .parse::<usize>()
                .map_err(|_| MediaError::ParseFailure)?;
            let text = shared.get(index).ok_or(MediaError::ParseFailure)?;
            ("string", text.clone())
        }
        Some("inlineStr") => ("string", cell.inline.clone()),
        Some("b") => (
            "bool",
            if cell.value == "1" { "true" } else { "false" }.to_owned(),
        ),
        Some("str") => ("string", cell.value.clone()),
        Some("e") => ("error", cell.value.clone()),
        _ => ("number", cell.value.clone()),
    };
    if value.is_empty() {
        return Ok(None);
    }
    Ok(Some(json!({
        "kind": kind,
        "ref": cell.reference,
        "value": truncate_chars(value, MAX_CELL_CHARS),
    })))
}

/// `A1:C3` -> (rows, cols) of the END anchor; single refs parse as
/// themselves.
fn dimension_of(reference: &str) -> Option<(usize, usize)> {
    let end = reference.rsplit(':').next().unwrap_or(reference);
    let split = end.find(|character: char| character.is_ascii_digit())?;
    let (letters, digits) = end.split_at(split);
    Some((digits.parse().ok()?, column_number(letters)?))
}

/// `C` -> 3.
fn column_number(letters: &str) -> Option<usize> {
    letters.bytes().try_fold(0, |accumulator, byte| {
        byte.is_ascii_uppercase()
            .then(|| accumulator * 26 + usize::from(byte - b'A' + 1))
    })
}
