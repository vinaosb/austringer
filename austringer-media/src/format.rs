//! The 34-format identifier contract: the digest section 6 CORE slate plus
//! the registered-for-refusal/compat rows (gif, jxl, mp3, ogg, hdf5).
//!
//! The wire form is the exact snake_case name of each variant (`"png"`,
//! `"geopackage"`, ...); [`FormatId::ALL`] lists every registered format in
//! slate order so registry-completeness checks (M6) can enumerate the whole
//! surface on day one. Deserialization of any other string is a typed serde
//! error - never a guess, never a panic.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A media format identity in the CORE slate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatId {
    /// PNG raster image (with APNG).
    Png,
    /// JPEG raster image.
    Jpeg,
    /// GIF raster image (decode/extract lane).
    Gif,
    /// WebP raster image.
    Webp,
    /// AVIF raster image (non-default `avif` feature).
    Avif,
    /// JPEG XL image (decode-only pending the adoption trigger).
    Jxl,
    /// SVG vector image.
    Svg,
    /// glTF / GLB 3D model.
    Gltf,
    /// Blender scene.
    Blend,
    /// PDF document (incl. PDF/A targets).
    Pdf,
    /// WAV audio.
    Wav,
    /// FLAC audio.
    Flac,
    /// Opus audio.
    Opus,
    /// MP3 audio (decode lane).
    Mp3,
    /// Ogg container (decode lane).
    Ogg,
    /// WebM video (AV1/VP9 payloads).
    Webm,
    /// Matroska video (AV1/VP9 payloads).
    Mkv,
    /// OOXML Word document.
    Docx,
    /// OOXML spreadsheet.
    Xlsx,
    /// OOXML presentation (generation via sandboxed pandoc).
    Pptx,
    /// Markdown text.
    Markdown,
    /// HTML5 text.
    Html,
    /// JSON (incl. JSONL lane).
    Json,
    /// CSV text.
    Csv,
    /// TOML text.
    Toml,
    /// STEP CAD exchange.
    Step,
    /// GeoTIFF / COG raster.
    Geotiff,
    /// GeoPackage vector (non-default `geovector` feature).
    Geopackage,
    /// GeoJSON vector.
    Geojson,
    /// KML vector.
    Kml,
    /// SQLite database.
    Sqlite,
    /// HDF5 - registered for probe + typed refusal only (digest: deferred).
    Hdf5,
    /// Apache Parquet columnar.
    Parquet,
    /// Arrow IPC columnar.
    Arrow,
}

impl FormatId {
    /// Every registered format (34), in slate order. The M6 registry
    /// completeness test enumerates this slice; the M27 honesty matrix is
    /// generated from the descriptors registered over it.
    pub const ALL: &'static [Self] = &[
        Self::Png,
        Self::Jpeg,
        Self::Gif,
        Self::Webp,
        Self::Avif,
        Self::Jxl,
        Self::Svg,
        Self::Gltf,
        Self::Blend,
        Self::Pdf,
        Self::Wav,
        Self::Flac,
        Self::Opus,
        Self::Mp3,
        Self::Ogg,
        Self::Webm,
        Self::Mkv,
        Self::Docx,
        Self::Xlsx,
        Self::Pptx,
        Self::Markdown,
        Self::Html,
        Self::Json,
        Self::Csv,
        Self::Toml,
        Self::Step,
        Self::Geotiff,
        Self::Geopackage,
        Self::Geojson,
        Self::Kml,
        Self::Sqlite,
        Self::Hdf5,
        Self::Parquet,
        Self::Arrow,
    ];
}

impl fmt::Display for FormatId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Jxl => "jxl",
            Self::Svg => "svg",
            Self::Gltf => "gltf",
            Self::Blend => "blend",
            Self::Pdf => "pdf",
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Opus => "opus",
            Self::Mp3 => "mp3",
            Self::Ogg => "ogg",
            Self::Webm => "webm",
            Self::Mkv => "mkv",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Markdown => "markdown",
            Self::Html => "html",
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Toml => "toml",
            Self::Step => "step",
            Self::Geotiff => "geotiff",
            Self::Geopackage => "geopackage",
            Self::Geojson => "geojson",
            Self::Kml => "kml",
            Self::Sqlite => "sqlite",
            Self::Hdf5 => "hdf5",
            Self::Parquet => "parquet",
            Self::Arrow => "arrow",
        };
        formatter.write_str(name)
    }
}
