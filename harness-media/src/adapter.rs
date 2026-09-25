//! Per-format adapters (plan todos M11-M23, M28-M30): each codec family
//! lands as its own feature-gated module implementing the M1 contract
//! (extract/generate/verify handlers + descriptor claims + projector
//! registration) on top of the M2 probe table, the M5 diff engine, and
//! the M6 executor. The first lane to land establishes the module; later
//! lanes append siblings and never touch each other's files.

#[cfg(feature = "svg")]
pub mod svg;

#[cfg(feature = "gltf")]
pub mod gltf;

#[cfg(feature = "gltf")]
mod gltf_container;
#[cfg(feature = "gltf")]
mod gltf_encode;
#[cfg(feature = "gltf")]
mod gltf_extract;
#[cfg(feature = "gltf")]
mod gltf_spec;
#[cfg(feature = "gltf")]
mod gltf_write;
#[cfg(feature = "pdf")]
pub mod pdf;
#[cfg(feature = "pdf")]
mod pdf_extract;
#[cfg(feature = "pdf")]
mod pdf_generate;
#[cfg(feature = "pdf")]
mod pdf_verify;

#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "audio")]
mod audio_decode;
#[cfg(feature = "audio")]
mod audio_flac;
#[cfg(feature = "audio")]
mod audio_synth;
#[cfg(feature = "audio")]
mod audio_wav;
#[cfg(feature = "text")]
pub mod text;

#[cfg(feature = "image")]
pub mod image;

// M18 office read lane: pure zip+quick-xml OOXML extract under the
// `office` feature (the M2 probe lane already carries the zip half).
#[cfg(feature = "office")]
pub mod office;

// M16 blend lane: subprocess tier (headless Blender via the M15 tool
// executor) - no codec dependency, so it is not feature-gated.
pub mod blend;

#[cfg(feature = "ffmpeg")]
pub mod ffmpeg;
