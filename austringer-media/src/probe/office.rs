//! The OOXML zip lane. The `PK\x03\x04` magic itself is dep-free; reading
//! the central directory to disambiguate docx/xlsx/pptx uses the zip
//! crate and is therefore gated behind the `office` feature - without it
//! a zip is honestly Unknown with a note naming the feature.

use crate::probe::{ProbeResult, unknown};

const ZIP_LOCAL_HEADER: &[u8] = b"PK\x03\x04";

#[cfg(feature = "office")]
pub(super) fn sniff(bytes: &[u8]) -> Option<ProbeResult> {
    use std::io::Read;

    use crate::format::FormatId;
    use crate::probe::hit;

    if !bytes.starts_with(ZIP_LOCAL_HEADER) {
        return None;
    }
    let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        return Some(unknown("zip container; central directory unreadable"));
    };
    let Ok(entry) = archive.by_name("[Content_Types].xml") else {
        return Some(unknown("zip container without OOXML [Content_Types].xml"));
    };
    let mut content = String::new();
    if entry.take(64 * 1024).read_to_string(&mut content).is_err() {
        return Some(unknown("zip container; [Content_Types].xml unreadable"));
    }
    if content.contains("wordprocessingml") {
        Some(hit(
            FormatId::Docx,
            "OOXML content types: wordprocessingml".into(),
        ))
    } else if content.contains("spreadsheetml") {
        Some(hit(
            FormatId::Xlsx,
            "OOXML content types: spreadsheetml".into(),
        ))
    } else if content.contains("presentationml") {
        Some(hit(
            FormatId::Pptx,
            "OOXML content types: presentationml".into(),
        ))
    } else {
        Some(unknown(
            "zip with [Content_Types].xml but no office document flavor",
        ))
    }
}

#[cfg(not(feature = "office"))]
pub(super) fn sniff(bytes: &[u8]) -> Option<ProbeResult> {
    if bytes.starts_with(ZIP_LOCAL_HEADER) {
        return Some(unknown(
            "zip container; OOXML disambiguation requires the `office` feature",
        ));
    }
    None
}
