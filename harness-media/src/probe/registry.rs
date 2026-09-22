//! The M2 probe registry: descriptors for all 34 formats with probe
//! Supported and honest typed-Unsupported placeholders for the
//! operations that arrive with the adapter todos (M11-M23, M28-M30).
//! M6 attaches executor entries to this same table - it never replaces
//! it.

use std::collections::BTreeMap;

use crate::descriptor::{FormatDescriptor, FormatRegistry, Operation, OpSupport};
use crate::format::FormatId;

/// A registry with a descriptor for every registered format.
#[must_use]
pub fn registry() -> FormatRegistry {
    let mut registry = FormatRegistry::new();
    for &(id, mime, extensions) in TABLE {
        registry.register(descriptor(id, mime, extensions));
    }
    registry
}

fn descriptor(id: FormatId, mime: &str, extensions: &[&str]) -> FormatDescriptor {
    let mut ops = BTreeMap::new();
    ops.insert(Operation::Probe, OpSupport::Supported);
    ops.insert(
        Operation::Extract,
        OpSupport::Unsupported {
            reason: "extract arrives with the per-format adapter todos (M11-M23, M28-M30)"
                .into(),
        },
    );
    ops.insert(
        Operation::Generate,
        OpSupport::Unsupported {
            reason: "generation arrives with the per-format adapter todos (M11-M23, M28-M30)"
                .into(),
        },
    );
    ops.insert(
        Operation::Verify,
        OpSupport::Unsupported {
            reason: "round-trip verification arrives with the M5 diff engine wiring (M6+)"
                .into(),
        },
    );
    FormatDescriptor {
        id,
        mime: mime.into(),
        extensions: extensions.iter().map(|extension| (*extension).to_string()).collect(),
        ops,
    }
}

/// (format, canonical mime, extensions) for the full slate.
const TABLE: &[(FormatId, &str, &[&str])] = &[
    (FormatId::Png, "image/png", &["png"]),
    (FormatId::Jpeg, "image/jpeg", &["jpeg", "jpg", "jfif"]),
    (FormatId::Gif, "image/gif", &["gif"]),
    (FormatId::Webp, "image/webp", &["webp"]),
    (FormatId::Avif, "image/avif", &["avif", "avifs"]),
    (FormatId::Jxl, "image/jxl", &["jxl"]),
    (FormatId::Svg, "image/svg+xml", &["svg", "svgz"]),
    (FormatId::Gltf, "model/gltf+json", &["gltf", "glb"]),
    (FormatId::Blend, "application/x-blender", &["blend"]),
    (FormatId::Pdf, "application/pdf", &["pdf"]),
    (FormatId::Wav, "audio/wav", &["wav", "wave"]),
    (FormatId::Flac, "audio/flac", &["flac"]),
    (FormatId::Opus, "audio/opus", &["opus"]),
    (FormatId::Mp3, "audio/mpeg", &["mp3"]),
    (FormatId::Ogg, "audio/ogg", &["ogg", "oga"]),
    (FormatId::Webm, "video/webm", &["webm"]),
    (FormatId::Mkv, "video/x-matroska", &["mkv"]),
    (
        FormatId::Docx,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        &["docx"],
    ),
    (
        FormatId::Xlsx,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        &["xlsx"],
    ),
    (
        FormatId::Pptx,
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        &["pptx"],
    ),
    (FormatId::Markdown, "text/markdown", &["md", "markdown"]),
    (FormatId::Html, "text/html", &["html", "htm"]),
    (FormatId::Json, "application/json", &["json", "jsonl"]),
    (FormatId::Csv, "text/csv", &["csv"]),
    (FormatId::Toml, "application/toml", &["toml"]),
    (FormatId::Step, "application/step", &["step", "stp"]),
    (FormatId::Geotiff, "image/tiff", &["tif", "tiff"]),
    (FormatId::Geopackage, "application/geopackage+sqlite3", &["gpkg"]),
    (FormatId::Geojson, "application/geo+json", &["geojson"]),
    (
        FormatId::Kml,
        "application/vnd.google-earth.kml+xml",
        &["kml"],
    ),
    (FormatId::Sqlite, "application/vnd.sqlite3", &["sqlite", "sqlite3", "db"]),
    (FormatId::Hdf5, "application/x-hdf5", &["h5", "hdf5"]),
    (FormatId::Parquet, "application/vnd.apache.parquet", &["parquet", "pq"]),
    (FormatId::Arrow, "application/x-arrow-ipc", &["arrow", "feather"]),
];
