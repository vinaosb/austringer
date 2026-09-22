//! Audio/video lanes: flac, ogg/opus, mp3 (ID3 + guarded frame sync),
//! RIFF dispatch (WAVE here, WEBP in the raster lane, everything else a
//! typed Unknown), and the EBML webm/mkv dispatch.

use crate::format::FormatId;
use crate::probe::{find_sub, hit, likely, ProbeResult};

const EBML: &[u8] = &[0x1A, 0x45, 0xDF, 0xA3];

pub(super) fn sniff(bytes: &[u8], hint: Option<FormatId>) -> Option<ProbeResult> {
    if bytes.starts_with(b"fLaC") {
        return Some(hit(FormatId::Flac, "fLaC STREAMINFO magic".into()));
    }
    if bytes.starts_with(b"OggS") {
        let window = bytes.get(..256).unwrap_or(bytes);
        if find_sub(window, b"OpusHead").is_some() {
            return Some(hit(FormatId::Opus, "ogg first page carries OpusHead".into()));
        }
        return Some(hit(FormatId::Ogg, "ogg container (non-Opus codec header)".into()));
    }
    if bytes.starts_with(b"RIFF") {
        if bytes.get(8..12) == Some(b"WAVE".as_slice()) {
            return Some(hit(FormatId::Wav, "RIFF container, WAVE form".into()));
        }
        return Some(ProbeResult::Unknown {
            note: "RIFF container; form not in the CORE slate or truncated".into(),
        });
    }
    if bytes.starts_with(b"ID3") || mp3_sync(bytes) {
        return Some(hit(
            FormatId::Mp3,
            if bytes.starts_with(b"ID3") {
                "ID3v2 tag"
            } else {
                "MPEG audio frame sync"
            }
            .into(),
        ));
    }
    if bytes.starts_with(EBML) {
        return Some(ebml(bytes, hint));
    }
    None
}

/// MPEG frame sync with the classic false positives closed: version and
/// layer bits must be non-reserved, bitrate index non-free/non-reserved,
/// samplerate non-reserved, and a UTF-16 BOM is never a sync word.
fn mp3_sync(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return false;
    }
    let (Some(&b1), Some(&b2)) = (bytes.get(1), bytes.get(2)) else {
        return false;
    };
    if bytes.first() != Some(&0xFF) || b1 & 0xE0 != 0xE0 {
        return false;
    }
    (b1 >> 3) & 0x03 != 0x01
        && (b1 >> 1) & 0x03 != 0x00
        && b2 >> 4 != 0x00
        && b2 >> 4 != 0x0F
        && (b2 >> 2) & 0x03 != 0x03
}

/// EBML magic: the earliest of the webm/matroska doctype strings in the
/// sniff window decides; with neither, only a name hint may claim one at
/// Likely, else typed Unknown.
fn ebml(bytes: &[u8], hint: Option<FormatId>) -> ProbeResult {
    let window = bytes.get(..256).unwrap_or(bytes);
    let webm = find_sub(window, b"webm");
    let matroska = find_sub(window, b"matroska");
    match (webm, matroska) {
        (Some(_), None) => hit(FormatId::Webm, "EBML magic; doctype webm".into()),
        (Some(w), Some(m)) if w < m => hit(FormatId::Webm, "EBML magic; doctype webm".into()),
        (None, Some(_)) | (Some(_), Some(_)) => {
            hit(FormatId::Mkv, "EBML magic; doctype matroska".into())
        }
        (None, None) => match hint {
            Some(FormatId::Webm) => likely(
                FormatId::Webm,
                "EBML magic, doctype not resolvable; name hint".into(),
            ),
            Some(FormatId::Mkv) => {
                likely(FormatId::Mkv, "EBML magic, doctype not resolvable; name hint".into())
            }
            _ => ProbeResult::Unknown {
                note: "EBML container; doctype not resolvable in the sniff window".into(),
            },
        },
    }
}
