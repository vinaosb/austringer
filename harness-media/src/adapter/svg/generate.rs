//! The svg writer: spec -> well-formed deterministic SVG 1.1. The XML
//! declaration, the namespace declaration, fixed attribute order per
//! element, and partial-escape for text content make output byte-stable
//! across runs (svg carries no clock fields, so the M1 determinism
//! contract holds unconditionally). Only SVG 1.1 basic shapes and
//! presentation attributes are emitted - no SVG 2 constructs, so no
//! fallback is owed (digest 3.2: SVG 2 is still CR; target 1.1).

use quick_xml::escape::partial_escape;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use super::shapes::{CommonAttrs, Shape, DEFAULT_FONT_FAMILY};
use super::spec::SvgSpec;
use super::SVG_NAMESPACE;
use crate::error::MediaError;
use crate::generate::GenerationSpec;

/// Generate deterministic svg bytes from a validated spec.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec does not satisfy the svg
/// contract (format mismatch, malformed params, semantic violations).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let svg = SvgSpec::from_generation(spec)?;
    let mut writer = Writer::new(Vec::new());
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|_| MediaError::ParseFailure)?;
    let mut root = BytesStart::new("svg");
    root.push_attribute(("xmlns", SVG_NAMESPACE));
    if let Some(numbers) = svg.view_box {
        root.push_attribute(("viewBox", coordinates(&numbers).as_str()));
    }
    if let Some(width) = svg.width {
        root.push_attribute(("width", number(width).as_str()));
    }
    if let Some(height) = svg.height {
        root.push_attribute(("height", number(height).as_str()));
    }
    writer
        .write_event(Event::Start(root))
        .map_err(|_| MediaError::ParseFailure)?;
    if let Some(title) = &svg.title {
        text_element(&mut writer, "title", title)?;
    }
    if let Some(desc) = &svg.desc {
        text_element(&mut writer, "desc", desc)?;
    }
    for shape in &svg.shapes {
        shape_element(&mut writer, shape)?;
    }
    writer
        .write_event(Event::End(BytesEnd::new("svg")))
        .map_err(|_| MediaError::ParseFailure)?;
    Ok(writer.into_inner())
}

fn number(value: f64) -> String {
    format!("{value}")
}

fn coordinates(numbers: &[f64]) -> String {
    numbers.iter().map(|value| number(*value)).collect::<Vec<_>>().join(" ")
}

fn push_common(start: &mut BytesStart<'_>, common: &CommonAttrs) {
    if let Some(id) = &common.id {
        start.push_attribute(("id", id.as_str()));
    }
    if let Some(class) = &common.class {
        start.push_attribute(("class", class.as_str()));
    }
    if let Some(transform) = &common.transform {
        start.push_attribute(("transform", transform.as_str()));
    }
}

fn push_presentation(start: &mut BytesStart<'_>, common: &CommonAttrs) {
    if let Some(fill) = &common.fill {
        start.push_attribute(("fill", fill.as_str()));
    }
    if let Some(stroke) = &common.stroke {
        start.push_attribute(("stroke", stroke.as_str()));
    }
    if let Some(stroke_width) = common.stroke_width {
        start.push_attribute(("stroke-width", number(stroke_width).as_str()));
    }
    if let Some(opacity) = common.opacity {
        start.push_attribute(("opacity", number(opacity).as_str()));
    }
}

fn empty_shape(writer: &mut Writer<Vec<u8>>, start: BytesStart<'_>) -> Result<(), MediaError> {
    writer
        .write_event(Event::Empty(start))
        .map_err(|_| MediaError::ParseFailure)
}

fn text_element(
    writer: &mut Writer<Vec<u8>>,
    tag: &str,
    content: &str,
) -> Result<(), MediaError> {
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(|_| MediaError::ParseFailure)?;
    writer
        .write_event(Event::Text(BytesText::from_escaped(partial_escape(content))))
        .map_err(|_| MediaError::ParseFailure)?;
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(|_| MediaError::ParseFailure)
}

fn shape_element(writer: &mut Writer<Vec<u8>>, shape: &Shape) -> Result<(), MediaError> {
    let common = shape.common();
    let mut start = BytesStart::new(shape.tag());
    push_common(&mut start, common);
    match shape {
        Shape::Rect { x, y, width, height, rx, ry, .. } => {
            start.push_attribute(("x", number(*x).as_str()));
            start.push_attribute(("y", number(*y).as_str()));
            start.push_attribute(("width", number(*width).as_str()));
            start.push_attribute(("height", number(*height).as_str()));
            if let Some(rx) = rx {
                start.push_attribute(("rx", number(*rx).as_str()));
            }
            if let Some(ry) = ry {
                start.push_attribute(("ry", number(*ry).as_str()));
            }
        }
        Shape::Circle { cx, cy, r, .. } => {
            start.push_attribute(("cx", number(*cx).as_str()));
            start.push_attribute(("cy", number(*cy).as_str()));
            start.push_attribute(("r", number(*r).as_str()));
        }
        Shape::Ellipse { cx, cy, rx, ry, .. } => {
            start.push_attribute(("cx", number(*cx).as_str()));
            start.push_attribute(("cy", number(*cy).as_str()));
            start.push_attribute(("rx", number(*rx).as_str()));
            start.push_attribute(("ry", number(*ry).as_str()));
        }
        Shape::Line { x1, y1, x2, y2, .. } => {
            start.push_attribute(("x1", number(*x1).as_str()));
            start.push_attribute(("y1", number(*y1).as_str()));
            start.push_attribute(("x2", number(*x2).as_str()));
            start.push_attribute(("y2", number(*y2).as_str()));
        }
        Shape::Polyline { points, .. } => {
            start.push_attribute(("points", coordinates(points).as_str()));
        }
        Shape::Polygon { points, .. } => {
            start.push_attribute(("points", coordinates(points).as_str()));
        }
        Shape::Path { d, .. } => {
            start.push_attribute(("d", d.as_str()));
        }
        Shape::Text { x, y, content, font_size, font_family, .. } => {
            start.push_attribute(("x", number(*x).as_str()));
            start.push_attribute(("y", number(*y).as_str()));
            if let Some(font_size) = font_size {
                start.push_attribute(("font-size", number(*font_size).as_str()));
            }
            let family = font_family.as_deref().unwrap_or(DEFAULT_FONT_FAMILY);
            start.push_attribute(("font-family", family));
            push_presentation(&mut start, common);
            writer
                .write_event(Event::Start(start))
                .map_err(|_| MediaError::ParseFailure)?;
            writer
                .write_event(Event::Text(BytesText::from_escaped(partial_escape(content))))
                .map_err(|_| MediaError::ParseFailure)?;
            return writer
                .write_event(Event::End(BytesEnd::new("text")))
                .map_err(|_| MediaError::ParseFailure);
        }
    }
    push_presentation(&mut start, common);
    empty_shape(writer, start)
}
