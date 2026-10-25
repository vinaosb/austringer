//! The EXIF/XMP sidecar layer (plan M28): EXIF via kamadak-exif (read
//! via its reader, TIFF payloads via its experimental writer - the
//! crate's sanctioned write surface), XMP as deterministic emitted XML.
//! Embedding: PNG eXIf chunk + iTXt `XML:com.adobe.xmp`; JPEG APP1
//! segments (`Exif\0\0` + the Adobe XMP namespace).

use exif::experimental::Writer as ExifWriter;
use exif::{Field, In, Tag, Value as ExifValue};
use serde_json::{Map, Value as Json, json};

use crate::error::MediaError;

use super::spec::{ExifSpec, XmpSpec};

/// PNG iTXt keyword carrying an XMP packet.
pub const XMP_KEYWORD: &str = "XML:com.adobe.xmp";

/// JPEG APP1 namespace prefix for XMP (NUL-terminated on the wire).
pub const JPEG_XMP_NS: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// EXIF `DateTime` is the determinism epoch rendered in EXIF's
/// `YYYY:MM:DD HH:MM:SS` form (never the ambient clock).
pub const EXIF_DATE_FORMAT: &str = "2026:01:01 00:00:00";

const MAX_EXIF_FIELDS: usize = 64;
const MAX_XMP_CHARS: usize = 1200;

/// Serialize a spec's EXIF fields into a TIFF payload (little-endian,
/// the form PNG's eXIf chunk and JPEG's APP1 both carry).
///
/// # Errors
/// [`MediaError::InvalidSpec`] never for validated specs; the kamadak
/// writer's io failure maps to it (output is an in-memory Vec).
pub fn exif_tiff(exif: &ExifSpec) -> Result<Vec<u8>, MediaError> {
    let mut writer = ExifWriter::new();
    let mut fields = Vec::new();
    if let Some(orientation) = exif.orientation {
        fields.push(Field {
            tag: Tag::Orientation,
            ifd_num: In::PRIMARY,
            value: ExifValue::Short(vec![orientation]),
        });
    }
    if let Some(description) = &exif.description {
        fields.push(ascii_field(Tag::ImageDescription, description));
    }
    if let Some(artist) = &exif.artist {
        fields.push(ascii_field(Tag::Artist, artist));
    }
    fields.push(ascii_field(Tag::DateTime, EXIF_DATE_FORMAT));
    for field in &fields {
        writer.push_field(field);
    }
    let mut out = Vec::new();
    writer
        .write(&mut std::io::Cursor::new(&mut out), false)
        .map_err(|_| MediaError::InvalidSpec)?;
    Ok(out)
}

fn ascii_field(tag: Tag, text: &str) -> Field {
    Field {
        tag,
        ifd_num: In::PRIMARY,
        value: ExifValue::Ascii(vec![format!("{text}\0").into_bytes()]),
    }
}

/// Parse a TIFF EXIF payload into the canonical extract subtree
/// (`{fields: {...}, unknown_tags: N}`), or `None` when it does not
/// parse (absence and corruption are both honestly absent here - the
/// container layer reports presence separately).
#[must_use]
pub fn parse_exif_tree(tiff: &[u8]) -> Option<Json> {
    let (fields, _little_endian) = exif::parse_exif(tiff).ok()?;
    let mut tree = Map::new();
    let mut unknown = 0_u64;
    for field in &fields {
        let name = match field.tag {
            Tag::Orientation => "orientation",
            Tag::ImageDescription => "image_description",
            Tag::Artist => "artist",
            Tag::DateTime => "date_time",
            _ => {
                unknown = unknown.saturating_add(1);
                continue;
            }
        };
        if tree.len() >= MAX_EXIF_FIELDS {
            unknown = unknown.saturating_add(1);
            continue;
        }
        let value = match &field.value {
            ExifValue::Ascii(entries) => entries
                .first()
                .map(|entry| {
                    let nul_len = entry.iter().rev().take_while(|byte| **byte == 0).count();
                    String::from_utf8_lossy(&entry[..entry.len() - nul_len]).into_owned()
                })
                .map_or_else(|| Json::Null, Json::String),
            other => other
                .get_uint(0)
                .map_or_else(|| Json::Null, |number| json!(number)),
        };
        tree.insert(name.to_owned(), value);
    }
    Some(json!({"fields": Json::Object(tree), "unknown_tags": unknown}))
}

/// Escape XML text content (the five predefined entities).
#[must_use]
pub fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Emit the deterministic XMP packet for a spec (fixed element order,
/// escaped values, xpacket wrapper - byte-stable across runs).
#[must_use]
pub fn xmp_packet(xmp: &XmpSpec) -> String {
    let mut out = String::from("<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    out.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n");
    out.push_str(" <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    out.push_str(
        "  <rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n",
    );
    if let Some(title) = &xmp.title {
        out.push_str(&format!(
            "   <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>\n",
            escape_xml(title)
        ));
    }
    if let Some(description) = &xmp.description {
        out.push_str(&format!(
            "   <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:description>\n",
            escape_xml(description)
        ));
    }
    out.push_str("  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n");
    out.push_str("<?xpacket end=\"w\"?>");
    out
}

/// Truncate an extracted XMP packet to the projection bound.
#[must_use]
pub fn bound_xmp(packet: &str) -> String {
    super::truncate_chars(packet.to_owned(), MAX_XMP_CHARS)
}
