//! Untrusted-wire → typed-Answer extraction (bounded; never panics on
//! hostile payloads).

use serde_json::Value;

use crate::provider::HoverText;
use crate::render::MAX_LOCATIONS;

/// One location exactly as the server reported it (0-based).
pub(crate) struct WireLoc {
    /// Document URI (`uri` or, for LocationLink, `targetUri`).
    pub uri: String,
    /// 0-based line of the range start.
    pub line0: u32,
    /// 0-based character of the range start.
    pub char0: u32,
}

/// Extracts locations from a definition/references response (`null`, a
/// single object, or an array), truncated to [`MAX_LOCATIONS`].
pub(crate) fn wire_locations(value: &Value) -> Vec<WireLoc> {
    let items: Vec<&Value> = match value {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().collect(),
        object @ Value::Object(_) => vec![object],
        _ => Vec::new(),
    };
    items
        .iter()
        .filter_map(|item| {
            let uri = item
                .get("uri")
                .or_else(|| item.get("targetUri"))
                .and_then(Value::as_str)?;
            let range = item.get("range").or_else(|| item.get("targetRange"))?;
            let start = range.get("start")?;
            Some(WireLoc {
                uri: uri.to_owned(),
                line0: uint_of(start.get("line")),
                char0: uint_of(start.get("character")),
            })
        })
        .take(MAX_LOCATIONS)
        .collect()
}

/// Extracts hover text from a hover response (MarkupContent, MarkedString,
/// or arrays thereof), already budget-bounded.
pub(crate) fn hover_text(value: &Value) -> Option<HoverText> {
    let contents = value.get("contents")?;
    let mut parts: Vec<String> = Vec::new();
    collect_hover_strings(contents, &mut parts);
    if parts.is_empty() {
        return None;
    }
    Some(HoverText::new(&parts.join("\n")))
}

fn collect_hover_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_hover_strings(item, out);
            }
        }
        Value::Object(_) => {
            if let Some(text) = value.get("value").and_then(Value::as_str) {
                out.push(text.to_owned());
            }
        }
        _ => {}
    }
}

fn uint_of(value: Option<&Value>) -> u32 {
    value
        .and_then(Value::as_i64)
        .map(|n| n.clamp(0, u32::MAX as i64) as u32)
        .unwrap_or(0)
}
