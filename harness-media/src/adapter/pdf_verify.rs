//! pdf verify: the M13 round-trip gate. Extracts the produced bytes,
//! projects BOTH sides to `{page_count, pages[{text}]}` with
//! whitespace-normalized text, and runs the M5 structural diff with
//! exact tolerances - empty diff is the PASS verdict (semantic
//! equivalence of the text layer, never byte identity).

use serde_json::{json, Map, Value};

use crate::diff::{structural_diff, RoundTripVerdict, Tolerances};
use crate::error::MediaError;
use crate::extract::ExtractBudgets;

use super::pdf::normalize_ws;
use super::pdf_extract::extract;

/// The pdf verify handler: whitespace-normalized text-layer diff of
/// produced bytes against the independent expected tree. Empty diff is
/// the PASS verdict.
pub fn verify(expected: &Value, bytes: &[u8]) -> Result<RoundTripVerdict, MediaError> {
    let extracted = extract(bytes, ExtractBudgets::default())?;
    let actual = comparable_tree(&extracted.tree);
    let diff = structural_diff(expected, &actual, &Tolerances::default())?;
    if diff.is_empty() {
        Ok(RoundTripVerdict::Pass)
    } else {
        Ok(RoundTripVerdict::Fail(diff))
    }
}

/// Projects a full extract tree down to the text-layer comparison form
/// (the same shape the independent [`super::spec_tree`] projection
/// emits).
fn comparable_tree(tree: &Value) -> Value {
    let page_count = tree["page_count"].as_u64().unwrap_or(0);
    let mut pages = Vec::new();
    if let Some(entries) = tree["pages"].as_array() {
        for entry in entries {
            let text = entry["text"].as_str().unwrap_or_default();
            let mut page = Map::new();
            page.insert("text".into(), Value::String(normalize_ws(text)));
            pages.push(Value::Object(page));
        }
    }
    json!({"page_count": page_count, "pages": pages})
}
