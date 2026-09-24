//! Per-format adapters (plan todos M11-M23, M28-M30): each codec family
//! lands as its own feature-gated module implementing the M1 contract
//! (extract/generate/verify handlers + descriptor claims + projector
//! registration) on top of the M2 probe table, the M5 diff engine, and
//! the M6 executor. The first lane to land establishes the module; later
//! lanes append siblings and never touch each other's files.

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
