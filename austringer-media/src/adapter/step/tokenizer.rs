//! The scanner half of the hand-rolled ISO-10303-21 tokenizer (plan
//! M20): a bounded, record-oriented pass over the ASCII exchange
//! grammar - strings with `''` escapes, `/* */` block remarks and `--`
//! line remarks, `#id` references, `.ENUM.` tokens, and
//! `;`-terminated records. Structure only: the per-record census
//! interpretation lives in [`super::interpret`]. Every cap (records,
//! input bytes, wall clock) trips the typed `ExtractTooLarge`;
//! malformed structure is the typed `ParseFailure`.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use super::interpret;
use crate::error::MediaError;
use crate::extract::ExtractBudgets;

/// Cap on captured string literals (bytes) - bounded memory on
/// hostile-but-magic-correct inputs.
const MAX_STRING_BYTES: usize = 4096;
/// Cap on DATA records scanned (the record cap of the depth/size-cap
/// contract).
const MAX_RECORDS: u64 = 1_048_576;

/// One product row of the assembly hierarchy skeleton.
pub(super) struct ProductRow {
    pub id: u64,
    pub name: String,
}

/// The census surface one scan produces (chain links are raw ids; the
/// skeleton resolution happens in [`super::extract`]).
pub(super) struct Census {
    pub file_description: Option<String>,
    pub file_schema: Option<String>,
    pub entity_count: u64,
    pub census: BTreeMap<String, u64>,
    pub products: Vec<ProductRow>,
    /// PRODUCT_DEFINITION_FORMATION records: (formation id, product id).
    pub formation_links: Vec<(u64, u64)>,
    /// PRODUCT_DEFINITION records: (definition id, formation id).
    pub definition_links: Vec<(u64, u64)>,
    pub units: BTreeSet<String>,
    pub bbox_min: Option<[f64; 3]>,
    pub bbox_max: Option<[f64; 3]>,
}

/// One scanned token (commas and whitespace are separators only).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    /// A word: identifier, `#ref`, number, or `.ENUM.`.
    Word(String),
    /// A decoded string literal.
    Str(String),
    /// A parenthesis.
    Paren(char),
}

impl Census {
    fn empty() -> Self {
        Self {
            file_description: None,
            file_schema: None,
            entity_count: 0,
            census: BTreeMap::new(),
            products: Vec::new(),
            formation_links: Vec::new(),
            definition_links: Vec::new(),
            units: BTreeSet::new(),
            bbox_min: None,
            bbox_max: None,
        }
    }
}

/// Scan `bytes` (already magic-checked) into the census surface.
///
/// # Errors
/// [`MediaError::ParseFailure`] on malformed structure (missing record
/// id, unterminated string or remark, truncation before the END
/// marker); [`MediaError::ExtractTooLarge`] when any cap or the
/// deadline trips.
pub(super) fn scan(bytes: &[u8], budgets: ExtractBudgets) -> Result<Census, MediaError> {
    let started = Instant::now();
    let mut census = Census::empty();
    let mut in_data = false;
    let mut records = 0u64;
    let mut index = 0usize;
    let mut finished = false;
    while index < bytes.len() && !finished {
        let (tokens, next, terminated) = record(bytes, index)?;
        index = next;
        records += 1;
        if records > MAX_RECORDS || started.elapsed() > budgets.deadline {
            return Err(MediaError::ExtractTooLarge);
        }
        if tokens.is_empty() {
            continue;
        }
        match keyword(&tokens) {
            Some("DATA") => in_data = true,
            Some("HEADER") | Some("ENDSEC") => in_data = false,
            Some("END-ISO-10303-21") => finished = true,
            Some("ISO-10303-21") => {}
            _ => {
                if !terminated {
                    return Err(MediaError::ParseFailure);
                }
                if in_data {
                    interpret::data_record(&tokens, &mut census)?;
                } else {
                    interpret::header_record(&tokens, &mut census);
                }
            }
        }
    }
    if !finished {
        return Err(MediaError::ParseFailure);
    }
    Ok(census)
}

/// Whether the record is a bare section keyword (and which).
fn keyword(tokens: &[Tok]) -> Option<&str> {
    match tokens {
        [Tok::Word(word)] => Some(word.as_str()),
        _ => None,
    }
}

/// Scan one record starting at `index`; returns its tokens, the next
/// index, and whether it was `;`-terminated.
fn record(bytes: &[u8], mut index: usize) -> Result<(Vec<Tok>, usize, bool), MediaError> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' | b',' => {
                flush(&mut word, &mut tokens);
                index += 1;
            }
            b';' => {
                flush(&mut word, &mut tokens);
                return Ok((tokens, index + 1, true));
            }
            b'\'' => {
                flush(&mut word, &mut tokens);
                let (text, next) = string(bytes, index)?;
                tokens.push(Tok::Str(text));
                index = next;
            }
            b'(' | b')' => {
                flush(&mut word, &mut tokens);
                tokens.push(Tok::Paren(byte as char));
                index += 1;
            }
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                flush(&mut word, &mut tokens);
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                flush(&mut word, &mut tokens);
                index += 2;
                loop {
                    if index + 1 >= bytes.len() {
                        return Err(MediaError::ParseFailure);
                    }
                    if bytes[index] == b'*' && bytes[index + 1] == b'/' {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
            }
            _ => {
                word.push(byte as char);
                index += 1;
            }
        }
    }
    flush(&mut word, &mut tokens);
    let terminated = !tokens.is_empty();
    Ok((tokens, index, terminated))
}

/// One `'...'` literal with `''` escapes, starting at the opening
/// quote; capture is capped at [`MAX_STRING_BYTES`].
fn string(bytes: &[u8], start: usize) -> Result<(String, usize), MediaError> {
    let mut text = String::new();
    let mut index = start + 1;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\'' {
            if bytes.get(index + 1) == Some(&b'\'') {
                text.push('\'');
                index += 2;
                continue;
            }
            return Ok((text, index + 1));
        }
        if text.len() < MAX_STRING_BYTES {
            text.push(byte as char);
        }
        index += 1;
    }
    Err(MediaError::ParseFailure)
}

fn flush(word: &mut String, tokens: &mut Vec<Tok>) {
    if !word.is_empty() {
        tokens.push(Tok::Word(std::mem::take(word)));
    }
}
