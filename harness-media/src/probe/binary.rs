//! Binary magic lanes: raster images, documents, and the flat binary
//! containers (blend, sqlite/geopackage, tiff, parquet, arrow, hdf5,
//! OLE2). Each lane is a prefix/offset check - identification only, no
//! decode. Disjoint signatures, so order inside this module is only for
//! readability.

use crate::format::FormatId;
use crate::probe::{hit, ProbeResult};

const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const JXL_CONTAINER: &[u8] = &[
    0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
];
const HDF5: &[u8] = &[0x89, 0x48, 0x44, 0x46, 0x0D, 0x0A, 0x1A, 0x0A];
const OLE2: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const SQLITE: &[u8] = b"SQLite format 3\x00";

pub(super) fn sniff(bytes: &[u8]) -> Option<ProbeResult> {
    if bytes.starts_with(PNG) {
        return Some(hit(FormatId::Png, "PNG magic".into()));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(hit(FormatId::Jpeg, "JPEG SOI magic".into()));
    }
    if bytes.starts_with(b"GIF8") {
        return Some(hit(FormatId::Gif, "GIF87a/89a magic".into()));
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP".as_slice()) {
        return Some(hit(FormatId::Webp, "RIFF container, WEBP form".into()));
    }
    if bytes.get(4..8) == Some(b"ftyp".as_slice())
        && bytes.get(8..12).is_some_and(|brand| brand == b"avif" || brand == b"avis")
    {
        return Some(hit(FormatId::Avif, "ftyp box with avif/avis brand".into()));
    }
    if bytes.starts_with(&[0xFF, 0x0A]) {
        return Some(hit(FormatId::Jxl, "JXL bare codestream magic".into()));
    }
    if bytes.starts_with(JXL_CONTAINER) {
        return Some(hit(FormatId::Jxl, "JXL container magic".into()));
    }
    if bytes.starts_with(b"glTF") && bytes.get(4..8) == Some(&[0x02, 0x00, 0x00, 0x00][..]) {
        return Some(hit(FormatId::Gltf, "GLB container magic + version 2".into()));
    }
    if bytes.starts_with(b"%PDF-") {
        return Some(hit(FormatId::Pdf, "%PDF- magic".into()));
    }
    if bytes.starts_with(b"ISO-10303-21;") {
        return Some(hit(FormatId::Step, "ISO-10303-21 header".into()));
    }
    if let Some(result) = blend(bytes) {
        return Some(result);
    }
    if bytes.starts_with(HDF5) {
        return Some(hit(FormatId::Hdf5, "HDF5 superblock magic".into()));
    }
    if bytes.len() >= 8 && bytes.starts_with(b"PAR1") && bytes.ends_with(b"PAR1") {
        return Some(hit(FormatId::Parquet, "PAR1 magic at head and foot".into()));
    }
    if bytes.starts_with(b"ARROW1") && bytes.get(6) == Some(&0xFF) {
        return Some(hit(FormatId::Arrow, "ARROW1 magic + 0xFF continuation".into()));
    }
    if let Some(result) = tiff(bytes) {
        return Some(result);
    }
    if bytes.starts_with(SQLITE) {
        if bytes.get(68..72) == Some(b"GPKG".as_slice()) {
            return Some(hit(
                FormatId::Geopackage,
                "sqlite container; GPKG application_id".into(),
            ));
        }
        return Some(hit(FormatId::Sqlite, "sqlite database header".into()));
    }
    if bytes.starts_with(OLE2) {
        return Some(ProbeResult::LegacyOle2 {
            note: "legacy OLE2 compound document (D0 CF 11 E0 A1 B1 1A E1): \
                   RED-listed legacy DOC/XLS/PPT - identified, never parsed; \
                   convert to OOXML out-of-band"
                .into(),
        });
    }
    None
}

/// Real 12-byte layout: `BLENDER` + ptr-size in {_,-} + endianness in
/// {v,V} + 3-char ASCII version (plan deviation recorded in the module
/// docs).
fn blend(bytes: &[u8]) -> Option<ProbeResult> {
    if !bytes.starts_with(b"BLENDER")
        || bytes.len() < 12
        || !matches!(bytes.get(7), Some(b'_') | Some(b'-'))
        || !matches!(bytes.get(8), Some(b'v') | Some(b'V'))
        || !bytes.get(9..12).is_some_and(|v| v.iter().all(u8::is_ascii_digit))
    {
        return None;
    }
    let version = bytes
        .get(9..12)
        .map_or_else(String::new, |v| String::from_utf8_lossy(v).into_owned());
    Some(hit(
        FormatId::Blend,
        format!("blend 12-byte header, version {version}"),
    ))
}

/// The tiff lane answers the GeoTIFF/COG registry row: geokey
/// verification is extract-lane business, so the note says so. BigTIFF
/// (version 0x2B) is named.
fn tiff(bytes: &[u8]) -> Option<ProbeResult> {
    let classic = bytes.get(0..4) == Some(&[0x49, 0x49, 0x2A, 0x00][..])
        || bytes.get(0..4) == Some(&[0x4D, 0x4D, 0x00, 0x2A][..]);
    let big = bytes.get(0..4) == Some(&[0x49, 0x49, 0x2B, 0x00][..])
        || bytes.get(0..4) == Some(&[0x4D, 0x4D, 0x00, 0x2B][..]);
    if classic {
        return Some(hit(
            FormatId::Geotiff,
            "tiff container (geokeys verified in the extract lane)".into(),
        ));
    }
    if big {
        return Some(hit(
            FormatId::Geotiff,
            "BigTIFF container (geokeys verified in the extract lane)".into(),
        ));
    }
    None
}
