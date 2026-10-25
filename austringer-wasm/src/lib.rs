//! # austringer-wasm - capability-scoped WASM plugin runtime
//!
//! Plan todo 45 (late wave): sandboxed WebAssembly plugins that register as
//! tools in the [T6] `austringer_core::tools` registry. The runtime is built on
//! the wasmtime engine (WASI-0.2-era line, pinned v38) and is compiled in
//! ONLY behind the `wasm-plugins` cargo feature - the default build of this
//! crate carries just the typed manifest/capability/interface-ladder surface
//! so no workspace default build ever pays for the engine.
//!
//! ## Divergence from full WIT (documented, per plan)
//!
//! The plan's binding text asks for a "simplified versioned ABI table, not
//! full WIT tooling". Concretely this crate embeds wasmtime **core modules**
//! (not component-model components): plugins are authored in WebAssembly
//! text/binary with a fixed, versioned import/export surface
//! ([`abi`] documents the whole table), and file access is **host-mediated**:
//! instead of a WASI instance with fd preopens, the guest calls the typed
//! `austringer.read_file` / `austringer.write_file` host functions, which resolve
//! *virtual* paths (`"<scope>/<relative>"`) strictly inside the host-granted
//! preopen scopes. "Nothing else visible" is enforced by construction: the
//! guest has no other syscall surface at all.
//!
//! ## Security model
//!
//! * **Memory**: a [`wasmtime::ResourceLimiter`] caps linear memory at 32 MiB
//!   ([`limits::PluginLimits`]); a plugin that exceeds it is terminated
//!   cleanly with the typed [`error::WasmError::MemoryCapExceeded`].
//! * **CPU**: every call runs on an explicit instruction-fuel budget
//!   (default 10M); exhaustion terminates the call with the typed
//!   [`error::WasmError::FuelExhausted`]. The infinite-loop fixture is killed
//!   by exactly this path.
//! * **Capabilities** (Zed model, research §23): plugins DECLARE
//!   `process-exec` / `download` / `package-install` in their manifest; a
//!   declared-but-not-granted capability fails the load with
//!   [`error::WasmError::CapabilityNotGranted`] - never a silent escalation.
//! * **Interface ladder** (Zed's 10-WIT-versions lesson): plugins declare
//!   their interface version; the host supports a RANGE
//!   ([`abi::INTERFACE_MIN`], [`abi::INTERFACE_MAX`]). Older interfaces keep
//!   running across host upgrades; too-old/too-new fail typed with guidance.
//! * **Untrusted output**: everything a plugin produces (tool results, log
//!   lines) is bounded at the ABI boundary (4 KiB regions, lossy UTF-8) and
//!   flows through the T6 registry's byte-cap/spill path like any tool; the
//!   T7 inspector chain applies downstream exactly as for built-in tools.
//!
//! ## Hot reload
//!
//! A swapped plugin file is picked up for NEW calls only; in-flight calls
//! finish on the old instance ([`plugin::PluginHandle`] snapshot semantics),
//! and a failed reload keeps the old instance serving
//! ([`plugin::ReloadEvent::Failed`]).
//!
//! [T6]: austringer_core::tools
//! [`wasmtime::ResourceLimiter`]: wasmtime::ResourceLimiter

#![deny(missing_docs)]

pub mod abi;
pub mod error;
pub mod limits;
pub mod manifest;

#[cfg(feature = "wasm-plugins")]
pub mod hostfns;
#[cfg(feature = "wasm-plugins")]
pub mod hosttypes;
#[cfg(feature = "wasm-plugins")]
pub mod loader;
#[cfg(feature = "wasm-plugins")]
pub mod plugin;
#[cfg(feature = "wasm-plugins")]
pub mod provider;
#[cfg(feature = "wasm-plugins")]
pub mod runtime;

#[cfg(all(test, feature = "wasm-plugins"))]
mod hostfns_tests;
#[cfg(feature = "wasm-plugins")]
mod hostlink;
