//! Magic-sniffing probe registry (plan todo M2): typed identification of
//! the 34-format CORE slate from bytes, with the file name as a tie-break
//! hint only.
//!
//! # Ambiguity contract
//!
//! Sniff order is fixed: binary magics (incl. RIFF/EBML/Ogg dispatch),
//! then the OOXML zip lane, then the XML root lane (svg/kml), then the
//! JSON lane (gltf partial sniff, geojson, json/jsonl), then text
//! heuristics (html > toml > csv > markdown when several fire). The
//! [`name_hint`](probe) argument is an extension that may only SELECT
//! among candidates the bytes already fired, at
//! [`Confidence::Likely`] - it never adds a candidate and never overrides
//! byte evidence (an `.svg` renamed `.kml` still sniffs svg). Unknown is
//! a typed result, never a guess. Nothing here parses beyond
//! identification headers: no full decode, no dimension walking, no
//! geokey loads.
//!
//! # Blend header deviation (recorded)
//!
//! The plan text orders the blend header fields as "2-char version +
//! pointer-size + endianness + flags"; the real 12-byte layout is
//! `BLENDER` + pointer-size [7] in {_,-} + endianness [8] in {v,V} +
//! 3-char version [9..12]. This module implements the real layout (the
//! plan's field set is otherwise honored in full).

use crate::format::FormatId;
use serde::{Deserialize, Serialize};

mod av;
mod binary;
mod office;
mod registry;
mod textlane;

pub use registry::registry;

/// How strongly the bytes prove an identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Byte magic or structural root proves the format.
    Certain,
    /// Leading content is consistent with the format and nothing else
    /// fired; a name hint may have broken a tie.
    Likely,
}

/// Typed probe outcome. Unknown is a typed fact - never a guess.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProbeResult {
    /// Bytes identified as one of the 34 registered formats.
    Identified {
        /// The identified format.
        format: FormatId,
        /// How strongly the bytes prove it.
        confidence: Confidence,
        /// Short human-readable evidence trail.
        note: String,
    },
    /// Legacy OLE2 compound document (`D0 CF 11 E0 A1 B1 1A E1`): typed
    /// RED identification. Never parsed - the digest RED-lists legacy
    /// DOC/XLS/PPT; convert out-of-band to the OOXML form instead.
    LegacyOle2 {
        /// Short human-readable evidence trail.
        note: String,
    },
    /// No lane matched; the note says what was tried.
    Unknown {
        /// Short human-readable evidence trail.
        note: String,
    },
}

/// Sniff `bytes` and return the typed identification. `name_hint` is the
/// file name (or bare extension) of the source, used only to break ties
/// at [`Confidence::Likely`] among candidates the bytes already fired.
#[must_use]
pub fn probe(bytes: &[u8], name_hint: Option<&str>) -> ProbeResult {
    let hint = name_hint.and_then(hint_format);
    binary::sniff(bytes)
        .or_else(|| av::sniff(bytes, hint))
        .or_else(|| office::sniff(bytes))
        .or_else(|| textlane::sniff(bytes, hint))
        .unwrap_or(ProbeResult::Unknown {
            note: "no registered format matched the bytes".into(),
        })
}

/// The hint map: only the formats whose lanes have genuine byte-level
/// ties. geojson is deliberately absent (Feature types are
/// content-defined) and no magic-lane format belongs here (bytes decide).
fn hint_format(name: &str) -> Option<FormatId> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
        "md" | "markdown" => Some(FormatId::Markdown),
        "html" | "htm" => Some(FormatId::Html),
        "csv" => Some(FormatId::Csv),
        "toml" => Some(FormatId::Toml),
        "kml" => Some(FormatId::Kml),
        "webm" => Some(FormatId::Webm),
        "mkv" => Some(FormatId::Mkv),
        _ => None,
    }
}

pub(super) fn hit(format: FormatId, note: String) -> ProbeResult {
    ProbeResult::Identified {
        format,
        confidence: Confidence::Certain,
        note,
    }
}

pub(super) fn likely(format: FormatId, note: String) -> ProbeResult {
    ProbeResult::Identified {
        format,
        confidence: Confidence::Likely,
        note,
    }
}

/// First offset of `needle` in `haystack`, if present.
pub(super) fn find_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

pub(super) fn unknown(note: &str) -> ProbeResult {
    ProbeResult::Unknown { note: note.into() }
}
