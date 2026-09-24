//! The canonical svg tree: the bounded structural summary BOTH the
//! extractor (walking real bytes) and the independent spec->tree
//! projection (computing from the spec) produce. Same shape, two
//! builders, separately tested - the M5 provenance rule's svg instance.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Number, Value};

use crate::adapter::svg::shapes::Shape;
use crate::adapter::svg::spec::SvgSpec;

/// The SMIL animation element names counted by
/// [`SvgTree::animation_elements`].
pub const ANIMATION_TAGS: [&str; 5] =
    ["animate", "animateMotion", "animateTransform", "animateColor", "set"];

/// The bounded extract tree (plan M11 field set): root attrs, element
/// census by tag, ids, classes, text content, embedded raster refs,
/// animation element count - plus title/desc so the generate round trip
/// can gate the accessibility elements it writes.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SvgTree {
    /// Count of SMIL animation elements.
    pub animation_elements: usize,
    /// Occurrences per element tag (root included).
    pub census: BTreeMap<String, usize>,
    /// Sorted unique `class` tokens.
    pub classes: Vec<String>,
    /// Sorted unique `href`/`xlink:href` values of `<image>` elements.
    pub embedded_rasters: Vec<String>,
    /// Sorted unique `id` values.
    pub ids: Vec<String>,
    /// Root attributes.
    pub root: RootAttrs,
    /// Whitespace-collapsed `<text>` contents, space-joined in document
    /// order.
    pub text: String,
    /// First `<title>` content (omitted when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// First `<desc>` content (omitted when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
}

/// Root element attributes. Numbers normalize to JSON floats on BOTH
/// builder sides (serde_json `Number::from_f64`); non-numeric values
/// (e.g. `width="100%"`) stay raw strings.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RootAttrs {
    /// `viewBox` as four floats (raw string when not four numbers).
    #[serde(rename = "viewBox", skip_serializing_if = "Option::is_none")]
    pub view_box: Option<ViewBox>,
    /// `width` (number or raw string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<Value>,
    /// `height` (number or raw string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<Value>,
}

/// A four-number `viewBox`.
#[derive(Debug, Clone, Serialize)]
pub struct ViewBox(pub [Number; 4]);

/// Parse a raw attribute value into the canonical number-or-string form.
#[must_use]
pub fn number_or_raw(raw: &str) -> Value {
    match raw.trim().parse::<f64>() {
        // from_f64 is None exactly for non-finite parses; those stay raw.
        Ok(number) => Number::from_f64(number).map_or_else(
            || Value::String(raw.to_owned()),
            Value::Number,
        ),
        Err(_) => Value::String(raw.to_owned()),
    }
}

/// Parse `viewBox`-style whitespace-separated numbers; four finite values
/// become a [`ViewBox`], anything else stays the raw string.
#[must_use]
pub fn parse_view_box(raw: &str) -> Option<ViewBox> {
    let numbers: Vec<f64> = raw.split_whitespace().filter_map(|token| token.parse().ok()).collect();
    boxed(numbers.try_into().ok()?)
}

/// Four finite f64s -> a [`ViewBox`] of canonical JSON numbers.
fn boxed(numbers: [f64; 4]) -> Option<ViewBox> {
    let converted: Option<Vec<Number>> = numbers.iter().map(|n| Number::from_f64(*n)).collect();
    Some(ViewBox(converted?.try_into().ok()?))
}

/// Whitespace-collapse: runs of whitespace become one space, ends
/// trimmed.
#[must_use]
pub fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate to at most `max` CHARS, at a char boundary.
#[must_use]
pub fn truncate_chars(raw: String, max: usize) -> String {
    match raw.char_indices().nth(max) {
        Some((index, _)) => raw[..index].to_owned(),
        None => raw,
    }
}

/// The INDEPENDENT spec->tree projection: the canonical tree the spec's
/// document must extract to, computed without the writer or extractor.
/// Generated documents embed no rasters and no animations by contract.
#[must_use]
pub fn spec_tree(spec: &SvgSpec) -> SvgTree {
    let mut tree = SvgTree::default();
    tree.root.view_box = spec.view_box.and_then(boxed);
    tree.root.width = spec.width.and_then(Number::from_f64).map(Value::Number);
    tree.root.height = spec.height.and_then(Number::from_f64).map(Value::Number);
    tree.census.insert("svg".to_owned(), 1);
    if let Some(title) = &spec.title {
        tree.census.insert("title".to_owned(), 1);
        tree.title = Some(collapse(title));
    }
    if let Some(desc) = &spec.desc {
        tree.census.insert("desc".to_owned(), 1);
        tree.desc = Some(collapse(desc));
    }
    let mut ids = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut texts = Vec::new();
    for shape in &spec.shapes {
        increment(&mut tree.census, shape.tag());
        let common = shape.common();
        if let Some(id) = &common.id {
            ids.insert(collapse(id));
        }
        for token in common.class.iter().flat_map(|class| class.split_whitespace()) {
            classes.insert(token.to_owned());
        }
        if let Shape::Text { content, .. } = shape {
            texts.push(collapse(content));
        }
    }
    tree.ids = ids.into_iter().collect();
    tree.classes = classes.into_iter().collect();
    tree.text = texts.join(" ");
    tree
}

fn increment(census: &mut BTreeMap<String, usize>, tag: &str) {
    *census.entry(tag.to_owned()).or_insert(0) += 1;
}

/// The deterministic one-line text projection of a tree (the caller
/// bounds it to [`super::TEXT_PROJECTION_MAX_CHARS`](crate::adapter::svg::TEXT_PROJECTION_MAX_CHARS)
/// chars).
#[must_use]
pub fn render_projection(tree: &SvgTree) -> String {
    let mut out = String::from("svg");
    if let Some(ViewBox(numbers)) = &tree.root.view_box {
        let joined: Vec<String> = numbers.iter().map(|number| number.to_string()).collect();
        out.push_str(&format!(" viewBox={}", joined.join(" ")));
    }
    for (label, value) in [("width", &tree.root.width), ("height", &tree.root.height)] {
        if let Some(value) = value {
            out.push_str(&format!(" {label}={value}"));
        }
    }
    let census: Vec<String> =
        tree.census.iter().map(|(tag, count)| format!("{tag}={count}")).collect();
    out.push_str(&format!("; elements={}", census.join(",")));
    if !tree.ids.is_empty() {
        out.push_str(&format!("; ids={}", tree.ids.join(",")));
    }
    if !tree.classes.is_empty() {
        out.push_str(&format!("; classes={}", tree.classes.join(",")));
    }
    if !tree.text.is_empty() {
        out.push_str(&format!("; text={}", tree.text));
    }
    if let Some(title) = &tree.title {
        out.push_str(&format!("; title={title}"));
    }
    if let Some(desc) = &tree.desc {
        out.push_str(&format!("; desc={desc}"));
    }
    out.push_str(&format!("; animations={}", tree.animation_elements));
    out.push_str(&format!("; rasters={}", tree.embedded_rasters.len()));
    out
}
