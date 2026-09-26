//! The census interpretation half of the ISO-10303-21 tokenizer (plan
//! M20): per-record interpretation over the scanner's token stream -
//! entity-type census counting, the PRODUCT chain links, units, the
//! CARTESIAN_POINT bbox, and the header facts. Caps trip the typed
//! `ExtractTooLarge`; malformed `#`-records are the typed
//! `ParseFailure`.

use super::tokenizer::{Census, Tok};
use crate::error::MediaError;

/// Cap on distinct entity types in the census.
const MAX_TYPES: usize = 4096;
/// Cap on products in the assembly skeleton.
const MAX_PRODUCTS: usize = 1024;

/// Whether a word is an entity TYPE (UPPER identifier) - the census key
/// form, always followed by `(` in the token stream.
fn is_type(word: &str, next: Option<&Tok>) -> bool {
    let mut chars = word.chars();
    matches!(chars.next(), Some('A'..='Z'))
        && chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        && matches!(next, Some(Tok::Paren('(')))
}

/// The TYPE tokens of a record, in order (complex instances contribute
/// every constituent).
fn types(tokens: &[Tok]) -> Vec<&str> {
    tokens
        .windows(2)
        .filter_map(|window| match (&window[0], &window[1]) {
            (Tok::Word(word), Tok::Paren('(')) if is_type(word, Some(&window[1])) => {
                Some(word.as_str())
            }
            _ => None,
        })
        .collect()
}

/// Header-entity facts (FILE_NAME is deliberately skipped: wall-clock
/// stamps never enter the canonical tree).
pub(super) fn header_record(tokens: &[Tok], census: &mut Census) {
    let name = tokens.first().and_then(|token| match token {
        Tok::Word(word) => Some(word.as_str()),
        _ => None,
    });
    let first_string = tokens.iter().find_map(|token| match token {
        Tok::Str(text) => Some(text.as_str()),
        _ => None,
    });
    match name {
        Some("FILE_DESCRIPTION") => census.file_description = first_string.map(str::to_owned),
        Some("FILE_SCHEMA") => census.file_schema = first_string.map(str::to_owned),
        _ => {}
    }
}

/// One DATA entity record: census counting plus the targeted chains.
///
/// # Errors
/// [`crate::error::MediaError::ParseFailure`] when the record shape is
/// malformed (`#`-record without a numeric id or without `=`);
/// [`crate::error::MediaError::ExtractTooLarge`] when the
/// census/products caps trip.
pub(super) fn data_record(tokens: &[Tok], census: &mut Census) -> Result<(), MediaError> {
    census.entity_count += 1;
    for kind in types(tokens) {
        *census.census.entry(kind.to_owned()).or_insert(0) += 1;
        if census.census.len() > MAX_TYPES {
            return Err(MediaError::ExtractTooLarge);
        }
    }
    let leading = tokens.first().and_then(|token| match token {
        Tok::Word(word) => Some(word.as_str()),
        _ => None,
    });
    let Some(leading) = leading.filter(|word| word.starts_with('#')) else {
        return Ok(());
    };
    let id = leading[1..]
        .parse::<u64>()
        .map_err(|_| MediaError::ParseFailure)?;
    if tokens.get(1) != Some(&Tok::Word("=".to_owned())) {
        return Err(MediaError::ParseFailure);
    }
    track_entity(id, tokens, census)
}

/// The first `#N` reference inside the entity's argument list (after
/// the opening paren - never the record's own `#id =` prefix).
fn first_reference(tokens: &[Tok]) -> Option<u64> {
    let args = tokens.iter().position(|tok| *tok == Tok::Paren('('))? + 1;
    tokens[args..].iter().find_map(|token| match token {
        Tok::Word(word) => word.strip_prefix('#').and_then(|digits| digits.parse().ok()),
        _ => None,
    })
}

/// The targeted per-type tracking (PRODUCT chain, units, points).
fn track_entity(id: u64, tokens: &[Tok], census: &mut Census) -> Result<(), MediaError> {
    let kinds = types(tokens);
    if kinds.contains(&"SI_UNIT") {
        let enums: Vec<&str> = tokens
            .iter()
            .filter_map(|token| match token {
                Tok::Word(word) => word.starts_with('.').then(|| word.trim_matches('.')),
                _ => None,
            })
            .filter(|entry| !entry.is_empty())
            .collect();
        if !enums.is_empty() {
            census.units.insert(enums.join("/"));
        }
    }
    if kinds.contains(&"CONVERSION_BASED_UNIT")
        && let Some(Tok::Str(unit)) = tokens.iter().find(|token| matches!(token, Tok::Str(_)))
    {
        census.units.insert(unit.clone());
    }
    if kinds.contains(&"CARTESIAN_POINT")
        && let Some(point) = numeric_tuple(tokens)
    {
        let (min, max) = match (census.bbox_min, census.bbox_max) {
            (Some(mut lo), Some(mut hi)) => {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(point[axis]);
                    hi[axis] = hi[axis].max(point[axis]);
                }
                (lo, hi)
            }
            _ => (point, point),
        };
        census.bbox_min = Some(min);
        census.bbox_max = Some(max);
    }
    if kinds.first() == Some(&"PRODUCT") {
        if census.products.len() >= MAX_PRODUCTS {
            return Err(MediaError::ExtractTooLarge);
        }
        let name = tokens.iter().find_map(|token| match token {
            Tok::Str(text) => Some(text.as_str()),
            _ => None,
        });
        census.products.push(super::tokenizer::ProductRow {
            id,
            name: name.unwrap_or_default().to_owned(),
        });
    }
    if kinds.first() == Some(&"PRODUCT_DEFINITION_FORMATION")
        && let Some(product) = first_reference(tokens)
    {
        census.formation_links.push((id, product));
    }
    if kinds.first() == Some(&"PRODUCT_DEFINITION")
        && let Some(formation) = first_reference(tokens)
    {
        census.definition_links.push((id, formation));
    }
    Ok(())
}

/// The last parenthesized group whose contents are exactly three
/// finite numbers (the CARTESIAN_POINT coordinate tuple form).
fn numeric_tuple(tokens: &[Tok]) -> Option<[f64; 3]> {
    let mut best: Option<[f64; 3]> = None;
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Tok::Paren('(') => depth += 1,
            Tok::Paren(')') => {
                if depth == 2
                    && let Some(start) = tokens[..index]
                        .iter()
                        .rposition(|tok| *tok == Tok::Paren('('))
                {
                    let group = &tokens[start + 1..index];
                    let words: Vec<&str> = group
                        .iter()
                        .filter_map(|tok| match tok {
                            Tok::Word(word) => Some(word.as_str()),
                            _ => None,
                        })
                        .collect();
                    if group.len() == 3 && words.len() == 3 {
                        let parsed: Option<Vec<f64>> =
                            words.iter().map(|word| word.parse::<f64>().ok()).collect();
                        if let Some(values) = parsed
                            && values.iter().all(|value| value.is_finite())
                        {
                            best = Some([values[0], values[1], values[2]]);
                        }
                    }
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    best
}

