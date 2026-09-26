//! The M2 probe registry: descriptors for all 34 formats with probe
//! Supported and honest typed-Unsupported placeholders for the
//! operations that arrive with the adapter todos (M11-M23, M28-M30).
//! M6 attaches executor entries to this same table - it never replaces
//! it.

use std::collections::BTreeMap;

use crate::descriptor::{FormatDescriptor, FormatRegistry, OpSupport, Operation};
use crate::format::FormatId;

/// A registry with a descriptor for every registered format. Landed
/// adapters flip their descriptor's op claims to `Supported` under their
/// feature (the M6 executor's descriptor/binding sync rule pairs this
/// with the handler bindings).
#[must_use]
pub fn registry() -> FormatRegistry {
    let mut registry = FormatRegistry::new();
    for &(id, mime, extensions) in TABLE {
        let mut entry = descriptor(id, mime, extensions);
        if cfg!(feature = "svg") && id == FormatId::Svg {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        if cfg!(feature = "gltf") && id == FormatId::Gltf {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        if cfg!(feature = "pdf") && id == FormatId::Pdf {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        // M14 audio: wav/flac = extract+generate+verify; mp3/ogg =
        // decode-only (encode redirects to M17, so no writer to verify).
        if cfg!(feature = "audio") {
            match id {
                FormatId::Wav | FormatId::Flac => {
                    for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                        entry.ops.insert(op, OpSupport::Supported);
                    }
                }
                FormatId::Mp3 | FormatId::Ogg => {
                    entry.ops.insert(Operation::Extract, OpSupport::Supported);
                }
                _ => {}
            }
        }
        // M30 text substrate: all five lanes are extract+generate+verify.
        if cfg!(feature = "text")
            && matches!(
                id,
                FormatId::Markdown
                    | FormatId::Html
                    | FormatId::Json
                    | FormatId::Csv
                    | FormatId::Toml
            )
        {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        // M28 image family: png/jpeg/webp = extract+generate+verify; jxl
        // is extract-only (generation gated on the digest adoption
        // trigger); avif flips under its own non-default feature.
        if cfg!(feature = "image") {
            match id {
                FormatId::Png | FormatId::Jpeg | FormatId::Webp => {
                    for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                        entry.ops.insert(op, OpSupport::Supported);
                    }
                }
                FormatId::Jxl => {
                    entry.ops.insert(Operation::Extract, OpSupport::Supported);
                    // Mirrors the executor's JXL_ADOPTION /
                    // JXL_EXTRACT_ONLY_VERIFY refusal texts (wiring.rs):
                    // one honesty row, two surfaces.
                    entry.ops.insert(
                        Operation::Generate,
                        OpSupport::Unsupported {
                            reason: "jxl generation is gated on the digest adoption trigger \
                                (Firefox 157 default landed; Chrome flip expected H2 2026) - \
                                extract-only until it fires"
                                .into(),
                        },
                    );
                    entry.ops.insert(
                        Operation::Verify,
                        OpSupport::Unsupported {
                            reason: "verify is a round-trip gate over a writer; jxl is \
                                extract-only (generation gated on the digest adoption trigger), \
                                so there is no writer to gate"
                                .into(),
                        },
                    );
                }
                _ => {}
            }
        }
        if cfg!(feature = "avif") && id == FormatId::Avif {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        // M22 columnar data lane: parquet + arrow-ipc round-trip (pure
        // tier, arrow-rs).
        if cfg!(feature = "data") && matches!(id, FormatId::Parquet | FormatId::Arrow) {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        // M18 office read lane: docx/xlsx/pptx extract is pure
        // zip+quick-xml under the `office` feature; the writers are
        // M19 (pptx generation is the pandoc subprocess), so
        // generate/verify stay typed-Unsupported with reasons naming
        // that lane (mirrors the ffmpeg row convention).
        if matches!(id, FormatId::Docx | FormatId::Xlsx | FormatId::Pptx) {
            if cfg!(feature = "office") {
                entry.ops.insert(Operation::Extract, OpSupport::Supported);
            }
            for (op, reason) in [
                (
                    Operation::Generate,
                    "generation arrives with the M19 office write lane (xlsx/docx pure \
                     writers; pptx via the pandoc subprocess)",
                ),
                (
                    Operation::Verify,
                    "round-trip verification arrives with the M19 office write lane \
                     (M18 extract + the M5 structural diff)",
                ),
            ] {
                entry.ops.insert(op, OpSupport::Unsupported { reason: reason.into() });
            }
        }
        // M16 blend: all three ops are decided and dispatched through
        // the blender subprocess lane (typed ToolUnavailable until the
        // tool is opted in; no codec feature - no cfg gate).
        if id == FormatId::Blend {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        // M20 step: extract is the pure hand-rolled tokenizer; generate
        // + verify are decided subprocess ops on the freecadcmd tool
        // lane (typed ToolUnavailable until the tool is opted in; no
        // codec feature - no cfg gate).
        if id == FormatId::Step {
            for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
                entry.ops.insert(op, OpSupport::Supported);
            }
        }
        // M17 ffmpeg lane: opus/webm/mkv run through the opt-in ffmpeg
        // subprocess adapter surface - the claims stay typed-Unsupported
        // (the OpRequest kinds do not carry the staging contract) but the
        // reasons name where the lane lives instead of a stale todo id.
        if matches!(id, FormatId::Opus | FormatId::Webm | FormatId::Mkv) {
            for (op, reason) in [
                (
                    Operation::Extract,
                    "extract runs through the opt-in ffmpeg subprocess adapter (M17: \
                     probe_av / extract_av over the enabled ffprobe + ffmpeg tools)",
                ),
                (
                    Operation::Generate,
                    "generate-class ops are TRANSCODE via the ffmpeg subprocess adapter \
                     (M17: GREEN targets only - no from-nothing synthesis)",
                ),
                (
                    Operation::Verify,
                    "verify is the ffmpeg probe(before/after) diff (M17: verify_transcode, \
                     lossy duration tolerance max(1 frame, 1%))",
                ),
            ] {
                entry.ops.insert(op, OpSupport::Unsupported { reason: reason.into() });
            }
        }
        registry.register(entry);
    }
    registry
}

fn descriptor(id: FormatId, mime: &str, extensions: &[&str]) -> FormatDescriptor {
    let mut ops = BTreeMap::new();
    ops.insert(Operation::Probe, OpSupport::Supported);
    ops.insert(
        Operation::Extract,
        OpSupport::Unsupported {
            reason: "extract arrives with the per-format adapter todos (M11-M23, M28-M30)".into(),
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
            reason: "round-trip verification arrives with the M5 diff engine wiring (M6+)".into(),
        },
    );
    FormatDescriptor {
        id,
        mime: mime.into(),
        extensions: extensions
            .iter()
            .map(|extension| (*extension).to_string())
            .collect(),
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
    (
        FormatId::Geopackage,
        "application/geopackage+sqlite3",
        &["gpkg"],
    ),
    (FormatId::Geojson, "application/geo+json", &["geojson"]),
    (
        FormatId::Kml,
        "application/vnd.google-earth.kml+xml",
        &["kml"],
    ),
    (
        FormatId::Sqlite,
        "application/vnd.sqlite3",
        &["sqlite", "sqlite3", "db"],
    ),
    (FormatId::Hdf5, "application/x-hdf5", &["h5", "hdf5"]),
    (
        FormatId::Parquet,
        "application/vnd.apache.parquet",
        &["parquet", "pq"],
    ),
    (
        FormatId::Arrow,
        "application/x-arrow-ipc",
        &["arrow", "feather"],
    ),
];
