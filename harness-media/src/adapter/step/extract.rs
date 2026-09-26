//! The pure STEP extract lane (plan M20): the hand-rolled tokenizer's
//! census becomes the canonical tree + the bounded structure-summary
//! projection. This lane NEVER touches freecadcmd - extract works with
//! the tool absent (the plan's "probe still works pure" posture,
//! extended to the whole census surface).

use serde_json::{json, Value};

use super::tokenizer;
use super::EXTRACT_SCHEMA;
use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted, TEXT_PROJECTION_MAX_CHARS};

/// The STEP magic every exchange structure starts with.
const MAGIC: &[u8] = b"ISO-10303-21;";

/// The pure extract handler (the M6 `ExtractHandler` signature): typed
/// census extraction under caller budgets.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-STEP bytes or malformed
/// structure; [`MediaError::ExtractTooLarge`] when the budgets or the
/// scanner caps trip.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    if !bytes.starts_with(MAGIC) {
        return Err(MediaError::ParseFailure);
    }
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let census = tokenizer::scan(bytes, budgets)?;
    Ok(Extracted {
        tree: canonical_tree(&census),
        text_projection: summary(&census),
        thumbnails: Vec::new(),
    })
}

/// Assemble the census into the canonical sorted-key tree.
fn canonical_tree(census: &tokenizer::Census) -> Value {
    let entity_census: Value = census
        .census
        .iter()
        .map(|(kind, count)| (kind.clone(), json!(count)))
        .collect::<serde_json::Map<String, Value>>()
        .into();
    let products: Vec<Value> = census
        .products
        .iter()
        .map(|row| {
            let formations: u64 = census
                .formation_links
                .iter()
                .filter(|(_, product)| *product == row.id)
                .count() as u64;
            let definitions: u64 = census
                .definition_links
                .iter()
                .filter(|(_, formation)| {
                    census
                        .formation_links
                        .iter()
                        .any(|(pdf, product)| pdf == formation && *product == row.id)
                })
                .count() as u64;
            json!({
                "definitions": definitions,
                "formations": formations,
                "id": row.id,
                "name": row.name,
            })
        })
        .collect();
    let bbox = match (census.bbox_min, census.bbox_max) {
        (Some(min), Some(max)) => json!({"max": max, "min": min}),
        _ => Value::Null,
    };
    json!({
        "bbox": bbox,
        "entity_census": entity_census,
        "entity_count": census.entity_count,
        "header": {
            "file_description": census.file_description,
            "file_schema": census.file_schema,
        },
        "products": products,
        "schema": EXTRACT_SCHEMA,
        "units": census.units.iter().cloned().collect::<Vec<String>>(),
    })
}

/// The bounded structure-summary projection (text-only first-class).
fn summary(census: &tokenizer::Census) -> String {
    let ap = census
        .file_schema
        .as_deref()
        .unwrap_or("unknown AP")
        .split_whitespace()
        .next()
        .unwrap_or("unknown AP");
    let mut lines = vec![format!(
        "step: {} - {} data entit{}, {} product(s){}",
        ap,
        census.entity_count,
        if census.entity_count == 1 { "y" } else { "ies" },
        census.products.len(),
        unit_note(census),
    )];
    for (index, product) in census.products.iter().take(16).enumerate() {
        let formations = census
            .formation_links
            .iter()
            .filter(|(_, product_id)| *product_id == product.id)
            .count();
        lines.push(format!(
            "- product #{}: {} ({} formation{})",
            product.id,
            product.name,
            formations,
            if formations == 1 { "" } else { "s" },
        ));
        if index == 15 && census.products.len() > 16 {
            lines.push(format!("... {} more", census.products.len() - 16));
        }
    }
    if let (Some(min), Some(max)) = (census.bbox_min, census.bbox_max) {
        lines.push(format!(
            "bbox: [{:.3}..{:.3}, {:.3}..{:.3}, {:.3}..{:.3}]",
            min[0], max[0], min[1], max[1], min[2], max[2],
        ));
    }
    let mut projection = lines.join("\n");
    projection.truncate(TEXT_PROJECTION_MAX_CHARS);
    projection
}

fn unit_note(census: &tokenizer::Census) -> String {
    if census.units.is_empty() {
        return String::new();
    }
    format!(", units: {}", census.units.iter().cloned().collect::<Vec<_>>().join(", "))
}
