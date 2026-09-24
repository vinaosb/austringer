//! Day-one wiring (plan M6): an [`AdapterEntry`] for EVERY registered
//! [`FormatId`] - all 34, on day one - so coverage gaps surface now, not
//! in MW6. Probe is bound to the M2 magic-sniffing handler everywhere;
//! extract/generate/verify are honest typed placeholders until their
//! adapter todos land, with the plan-decided subprocess lanes bound as
//! [`OpBinding::Subprocess`] (typed `ToolUnavailable` until M15 wires
//! the tool executor).
//!
//! ## Day-one table decisions (recorded)
//!
//! - Entry tier = the lane of the format's GENERATION op when one is
//!   planned in this crate; formats whose generation is a redirect to
//!   another format's executor (mp3/ogg encode -> ffmpeg, M17) take the
//!   tier of their own extract adapter. `blend`/`pptx`/`step`/`geotiff`
//!   (generation via blender/pandoc/freecadcmd/gdal) and
//!   `opus`/`webm`/`mkv` (decode + encode via ffmpeg) are
//!   [`AdapterTier::Subprocess`]; everything else is `Pure`.
//! - gif: probe + native vision delivery only - extract and generation
//!   are PERMANENT typed refusals (M28 honesty row), not
//!   waiting-for-the-adapter placeholders.
//! - jxl: extract-only; generation is gated on the digest adoption
//!   trigger (Firefox 157 default landed; Chrome flip expected H2 2026).
//! - hdf5: deferred by digest section 6 - probe + typed refusal only;
//!   the sanctioned future path is the interface-2 WASM media-adapter
//!   tier.
//! - mp3/ogg encode: typed redirect to the ffmpeg subprocess lane (no
//!   pure-Rust encoder is claimed); mp3/ogg decode stays a pure-lane
//!   placeholder (symphonia, M14).

use super::binding::{
    AdapterEntry, AdapterOps, ExtractHandler, GenerateHandler, OpBinding, ProbeHandler,
    VerifyHandler,
};
use super::registry::{ExecutorRegistry, ExecutorRegistryBuilder};
use super::types::AdapterTier;
use crate::descriptor::FormatDescriptor;
use crate::format::FormatId;
use crate::probe;

const PROBE_ENGINE: &str = "harness-media::probe";

const GIF_VISION_ONLY: &str = "gif is probe + native vision delivery only (the ImageBlock path \
     is unchanged); structural extract and generation are deliberately not gif lanes (M28 \
     honesty row)";

const JXL_ADOPTION: &str = "jxl generation is gated on the digest adoption trigger (Firefox 157 \
     default landed; Chrome flip expected H2 2026) - extract-only until it fires";

const JXL_EXTRACT_ONLY_VERIFY: &str = "verify is a round-trip gate over a writer; jxl is \
     extract-only (generation gated on the digest adoption trigger), so there is no writer to \
     gate";

const MP3_OGG_ENCODE_REDIRECT: &str =
    "encode-redirect: the pure tier never claims mp3/vorbis encode; encoding routes to the \
     ffmpeg subprocess lane (M17)";

const HDF5_DEFERRED: &str = "hdf5 is deferred by digest section 6 (single-vendor stewardship, \
     C-bindings-only Rust story; Parquet covers the actual need); the sanctioned future path is \
     the interface-2 WASM media-adapter tier";

const SQLITE_NO_GENERATION: &str =
    "sqlite has no generation lane: it is registered for probe + census extract only (M29)";

fn unsupported<H>(reason: &str) -> OpBinding<H> {
    OpBinding::Unsupported {
        reason: reason.to_owned(),
    }
}

fn tool_binding<H>(tool: &str) -> OpBinding<H> {
    OpBinding::Subprocess {
        tool: tool.to_owned(),
        install_hint: format!(
            "opt-in external tool, disabled by default; the pinned {tool} manifest and \
             installer arrive with the M15 tool executor"
        ),
    }
}

fn probe_binding() -> OpBinding<ProbeHandler> {
    OpBinding::Pure {
        engine: PROBE_ENGINE.to_owned(),
        handler: probe::probe,
    }
}

fn lane(id: FormatId) -> &'static str {
    match id {
        FormatId::Svg => "M11 (svg, pure tier)",
        FormatId::Gltf => "M12 (gltf, pure tier)",
        FormatId::Pdf => "M13 (pdf, pure tier)",
        FormatId::Wav | FormatId::Flac | FormatId::Mp3 | FormatId::Ogg => "M14 (audio, pure tier)",
        FormatId::Png | FormatId::Jpeg | FormatId::Webp | FormatId::Avif | FormatId::Jxl => {
            "M28 (image family)"
        }
        FormatId::Gif => "M28 (image family)",
        FormatId::Hdf5 => "M23 (honest-refusal row)",
        FormatId::Blend => "M16 (blender subprocess)",
        FormatId::Opus | FormatId::Webm | FormatId::Mkv => "M17 (ffmpeg subprocess)",
        FormatId::Docx | FormatId::Xlsx | FormatId::Pptx => "M18 (office read, pure tier)",
        FormatId::Markdown | FormatId::Html | FormatId::Json | FormatId::Csv | FormatId::Toml => {
            "M30 (text substrate)"
        }
        FormatId::Step => "M20 (step extract, pure tier)",
        FormatId::Geotiff => "M21 (geotiff read, pure tier)",
        FormatId::Geopackage | FormatId::Geojson | FormatId::Kml | FormatId::Sqlite => {
            "M29 (geovector + sqlite)"
        }
        FormatId::Parquet | FormatId::Arrow => "M22 (columnar, pure tier)",
    }
}

fn pending<H>(id: FormatId) -> OpBinding<H> {
    unsupported(&format!("arrives with the {} adapter todo", lane(id)))
}

// M12 gltf lane: pure handlers bound under the `gltf` feature; without
// it the honest pending placeholder stays.
#[cfg(feature = "gltf")]
fn gltf_extract_binding() -> OpBinding<ExtractHandler> {
    OpBinding::Pure {
        engine: crate::adapter::gltf::ENGINE.to_owned(),
        handler: crate::adapter::gltf::extract,
    }
}

#[cfg(not(feature = "gltf"))]
fn gltf_extract_binding() -> OpBinding<ExtractHandler> {
    pending(FormatId::Gltf)
}

#[cfg(feature = "gltf")]
fn gltf_generate_binding() -> OpBinding<GenerateHandler> {
    OpBinding::Pure {
        engine: crate::adapter::gltf::ENGINE.to_owned(),
        handler: crate::adapter::gltf::generate,
    }
}

#[cfg(not(feature = "gltf"))]
fn gltf_generate_binding() -> OpBinding<GenerateHandler> {
    pending(FormatId::Gltf)
}

#[cfg(feature = "gltf")]
fn gltf_verify_binding() -> OpBinding<VerifyHandler> {
    OpBinding::Pure {
        engine: crate::adapter::gltf::ENGINE.to_owned(),
        handler: crate::adapter::gltf::verify,
    }
}

#[cfg(not(feature = "gltf"))]
fn gltf_verify_binding() -> OpBinding<VerifyHandler> {
    pending(FormatId::Gltf)
}


// M13 pdf lane: pure handlers bound under the `pdf` feature; without it
// the honest pending placeholder stays.
#[cfg(feature = "pdf")]
fn pdf_extract_binding() -> OpBinding<ExtractHandler> {
    OpBinding::Pure {
        engine: crate::adapter::pdf::EXTRACT_ENGINE.to_owned(),
        handler: crate::adapter::pdf::extract,
    }
}

#[cfg(not(feature = "pdf"))]
fn pdf_extract_binding() -> OpBinding<ExtractHandler> {
    pending(FormatId::Pdf)
}

#[cfg(feature = "pdf")]
fn pdf_generate_binding() -> OpBinding<GenerateHandler> {
    OpBinding::Pure {
        engine: crate::adapter::pdf::GENERATE_ENGINE.to_owned(),
        handler: crate::adapter::pdf::generate,
    }
}

#[cfg(not(feature = "pdf"))]
fn pdf_generate_binding() -> OpBinding<GenerateHandler> {
    pending(FormatId::Pdf)
}

#[cfg(feature = "pdf")]
fn pdf_verify_binding() -> OpBinding<VerifyHandler> {
    OpBinding::Pure {
        engine: crate::adapter::pdf::VERIFY_ENGINE.to_owned(),
        handler: crate::adapter::pdf::verify,
    }
}

#[cfg(not(feature = "pdf"))]
fn pdf_verify_binding() -> OpBinding<VerifyHandler> {
    pending(FormatId::Pdf)
}

fn tier(id: FormatId) -> AdapterTier {
    match id {
        FormatId::Blend
        | FormatId::Pptx
        | FormatId::Step
        | FormatId::Geotiff
        | FormatId::Opus
        | FormatId::Webm
        | FormatId::Mkv => AdapterTier::Subprocess,
        _ => AdapterTier::Pure,
    }
}

// M11 svg lane: pure handlers bound under the `svg` feature; without it
// the honest typed placeholder stands (mirrors the gltf/pdf lanes).
#[cfg(feature = "svg")]
fn svg_extract_binding() -> OpBinding<ExtractHandler> {
    OpBinding::Pure {
        engine: crate::adapter::svg::ENGINE.to_owned(),
        handler: crate::adapter::svg::extract,
    }
}
#[cfg(not(feature = "svg"))]
fn svg_extract_binding() -> OpBinding<ExtractHandler> {
    pending(FormatId::Svg)
}

#[cfg(feature = "svg")]
fn svg_generate_binding() -> OpBinding<GenerateHandler> {
    OpBinding::Pure {
        engine: crate::adapter::svg::ENGINE.to_owned(),
        handler: crate::adapter::svg::generate,
    }
}
#[cfg(not(feature = "svg"))]
fn svg_generate_binding() -> OpBinding<GenerateHandler> {
    pending(FormatId::Svg)
}

#[cfg(feature = "svg")]
fn svg_verify_binding() -> OpBinding<VerifyHandler> {
    OpBinding::Pure {
        engine: crate::adapter::svg::ENGINE.to_owned(),
        handler: crate::adapter::svg::verify,
    }
}
#[cfg(not(feature = "svg"))]
fn svg_verify_binding() -> OpBinding<VerifyHandler> {
    pending(FormatId::Svg)
}

// M30 text substrate: pure handlers bound under the `text` feature;
// without it the honest pending placeholder stays (mirrors the
// svg/gltf/pdf/audio lanes).
#[cfg(feature = "text")]
fn text_extract_binding(id: FormatId) -> OpBinding<ExtractHandler> {
    let (engine, handler): (&str, ExtractHandler) = match id {
        FormatId::Markdown => {
            (crate::adapter::text::markdown::ENGINE, crate::adapter::text::markdown::extract)
        }
        FormatId::Html => {
            (crate::adapter::text::html::ENGINE, crate::adapter::text::html::extract)
        }
        FormatId::Json => {
            (crate::adapter::text::json::ENGINE, crate::adapter::text::json::extract)
        }
        FormatId::Csv => (crate::adapter::text::csv::ENGINE, crate::adapter::text::csv::extract),
        FormatId::Toml => {
            (crate::adapter::text::toml::ENGINE, crate::adapter::text::toml::extract)
        }
        _ => return pending(id),
    };
    OpBinding::Pure { engine: engine.to_owned(), handler }
}

#[cfg(feature = "text")]
fn text_generate_binding(id: FormatId) -> OpBinding<GenerateHandler> {
    let (engine, handler): (&str, GenerateHandler) = match id {
        FormatId::Markdown => {
            (crate::adapter::text::markdown::ENGINE, crate::adapter::text::markdown::generate)
        }
        FormatId::Html => {
            (crate::adapter::text::html::ENGINE, crate::adapter::text::html::generate)
        }
        FormatId::Json => {
            (crate::adapter::text::json::ENGINE, crate::adapter::text::json::generate)
        }
        FormatId::Csv => (crate::adapter::text::csv::ENGINE, crate::adapter::text::csv::generate),
        FormatId::Toml => {
            (crate::adapter::text::toml::ENGINE, crate::adapter::text::toml::generate)
        }
        _ => return pending(id),
    };
    OpBinding::Pure { engine: engine.to_owned(), handler }
}

#[cfg(feature = "text")]
fn text_verify_binding(id: FormatId) -> OpBinding<VerifyHandler> {
    let (engine, handler): (&str, VerifyHandler) = match id {
        FormatId::Markdown => {
            (crate::adapter::text::markdown::ENGINE, crate::adapter::text::markdown::verify)
        }
        FormatId::Html => {
            (crate::adapter::text::html::ENGINE, crate::adapter::text::html::verify)
        }
        FormatId::Json => {
            (crate::adapter::text::json::ENGINE, crate::adapter::text::json::verify)
        }
        FormatId::Csv => (crate::adapter::text::csv::ENGINE, crate::adapter::text::csv::verify),
        FormatId::Toml => {
            (crate::adapter::text::toml::ENGINE, crate::adapter::text::toml::verify)
        }
        _ => return pending(id),
    };
    OpBinding::Pure { engine: engine.to_owned(), handler }
}

fn extract_binding(id: FormatId) -> OpBinding<ExtractHandler> {
    match id {
        FormatId::Pdf => pdf_extract_binding(),
        FormatId::Markdown | FormatId::Html | FormatId::Json | FormatId::Csv
        | FormatId::Toml => {
            #[cfg(feature = "text")]
            {
                text_extract_binding(id)
            }
            #[cfg(not(feature = "text"))]
            {
                pending(id)
            }
        }
        FormatId::Svg => svg_extract_binding(),
        FormatId::Gltf => gltf_extract_binding(),
        FormatId::Gif => unsupported(GIF_VISION_ONLY),
        FormatId::Hdf5 => unsupported(HDF5_DEFERRED),
        FormatId::Blend => tool_binding("blender"),
        FormatId::Opus | FormatId::Webm | FormatId::Mkv => tool_binding("ffmpeg"),
        _ => pending(id),
    }
}

fn generate_binding(id: FormatId) -> OpBinding<GenerateHandler> {
    match id {
        FormatId::Pdf => pdf_generate_binding(),
        FormatId::Markdown | FormatId::Html | FormatId::Json | FormatId::Csv
        | FormatId::Toml => {
            #[cfg(feature = "text")]
            {
                text_generate_binding(id)
            }
            #[cfg(not(feature = "text"))]
            {
                pending(id)
            }
        }
        FormatId::Svg => svg_generate_binding(),
        FormatId::Gltf => gltf_generate_binding(),
        FormatId::Gif => unsupported(GIF_VISION_ONLY),
        FormatId::Jxl => unsupported(JXL_ADOPTION),
        FormatId::Hdf5 => unsupported(HDF5_DEFERRED),
        FormatId::Sqlite => unsupported(SQLITE_NO_GENERATION),
        FormatId::Mp3 | FormatId::Ogg => unsupported(MP3_OGG_ENCODE_REDIRECT),
        FormatId::Blend => tool_binding("blender"),
        FormatId::Pptx => tool_binding("pandoc"),
        FormatId::Step => tool_binding("freecadcmd"),
        FormatId::Geotiff => tool_binding("gdal"),
        FormatId::Opus | FormatId::Webm | FormatId::Mkv => tool_binding("ffmpeg"),
        _ => pending(id),
    }
}

fn verify_binding(id: FormatId) -> OpBinding<VerifyHandler> {
    match id {
        FormatId::Pdf => pdf_verify_binding(),
        FormatId::Markdown | FormatId::Html | FormatId::Json | FormatId::Csv
        | FormatId::Toml => {
            #[cfg(feature = "text")]
            {
                text_verify_binding(id)
            }
            #[cfg(not(feature = "text"))]
            {
                pending(id)
            }
        }
        FormatId::Svg => svg_verify_binding(),
        FormatId::Gltf => gltf_verify_binding(),
        FormatId::Gif => unsupported(GIF_VISION_ONLY),
        FormatId::Jxl => unsupported(JXL_EXTRACT_ONLY_VERIFY),
        FormatId::Hdf5 => unsupported(HDF5_DEFERRED),
        _ => pending(id),
    }
}

fn entry(id: FormatId, descriptor: FormatDescriptor) -> AdapterEntry {
    AdapterEntry {
        descriptor,
        tier: tier(id),
        ops: AdapterOps {
            probe: probe_binding(),
            extract: extract_binding(id),
            generate: generate_binding(id),
            verify: verify_binding(id),
        },
    }
}

/// The day-one registry: every registered format wired with a bound
/// probe and honest typed placeholders everywhere else. Built from the
/// M2 descriptor table (the executor attaches to it, never replaces it);
/// any builder rejection here fails the registry-completeness test,
/// which enumerates all 34 entries - never silently absorbed.
#[must_use]
pub fn registry() -> ExecutorRegistry {
    let mut builder = ExecutorRegistryBuilder::new();
    for (id, descriptor) in probe::registry().iter() {
        let _ = builder.register(entry(id, descriptor.clone()));
    }
    builder.build()
}
