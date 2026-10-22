//! # Tiered executor dispatch + producer disclosure (plan M6)
//!
//! The single dispatch authority for media operations. An
//! [`ExecutorRegistry`] maps every [`FormatId`](crate::format::FormatId)
//! to an [`AdapterEntry`] - the format's tier
//! ([`AdapterTier`]: Pure | Subprocess | Wasm), its static contract (the
//! M2 descriptor table), and its per-op bindings
//! ([`OpBinding`]: pure handler | subprocess lane | typed-unsupported
//! with the honest reason).
//!
//! ## Dispatch contract
//!
//! [`ExecutorRegistry::dispatch`] takes a typed [`OpRequest`] and
//! returns a [`Produced`] - the artifact INSEPARABLE from its
//! [`ProducerInfo`] disclosure (tier, engine, version, command).
//! A produced-without-producer value is unrepresentable: `Produced` has
//! private fields and exactly one constructor (see the compile-fail
//! proof on the type). Refusals are typed
//! [`MediaError`](crate::error::MediaError)s: unknown format,
//! [`UnsupportedOperation`](crate::error::MediaError::UnsupportedOperation)
//! with the verbatim reason, or - for the decided subprocess lanes - the
//! M15 tool executor's routing
//! ([`ToolUnavailable`](crate::error::MediaError::ToolUnavailable) with
//! tool + install hint only when the tool is genuinely not
//! enabled/installed; an available tool reports its pending adapter).
//!
//! ## Day one
//!
//! [`registry()`] wires ALL 34 registered formats: probe bound to the
//! M2 magic sniffing engine everywhere; extract/generate/verify as
//! honest typed placeholders citing their adapter todos (M11-M23,
//! M28-M30), with the plan-decided subprocess lanes (blender, pandoc,
//! freecadcmd, gdal, ffmpeg) bound as `Subprocess` and the permanent
//! honesty rows (gif vision-only, jxl adoption-gated, hdf5 deferred by
//! digest section 6, mp3/ogg encode-redirect) wired as first-class
//! refusals. The registry-completeness test enumerates the whole slate
//! so coverage gaps surface now, not in MW6. M31 registers these same
//! ops as engine tools - this registry stays the dispatch authority.
//! Subprocess dispatch consults the M15
//! [`ToolExecutor`](crate::tool::ToolExecutor) (all tools disabled by
//! default; see [`crate::tool`]).
//!
//! ## WASM seam
//!
//! [`AdapterTier::Wasm`] is declared for the interface-2 media-adapter
//! ladder (see harness-wasm `abi` docs: host fns `read_media` /
//! `write_media`, spec + manifest validation only in v0.2); no Wasm
//! bindings are registered today.

mod binding;
mod registry;
mod types;
mod wiring;

pub use binding::{
    AdapterEntry, AdapterOps, ExtractHandler, GenerateHandler, OpBinding, ProbeHandler, SubHandler,
    VerifyHandler,
};
pub use registry::{ExecutorRegistry, ExecutorRegistryBuilder, ExecutorWiringError};
pub use types::{AdapterTier, OpOutput, OpRequest, Produced, ProducerInfo};
pub use wiring::registry;
