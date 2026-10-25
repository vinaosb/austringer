//! Shared helpers for the austringer-wasm integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use austringer_wasm::error::WasmError;
use austringer_wasm::hostfns::{PluginGrant, ResolvedScope};
use austringer_wasm::limits::PluginLimits;
use austringer_wasm::manifest::PreopenMode;
use austringer_wasm::plugin::PluginHandle;

/// Absolute path of a fixture plugin under `tests/fixtures/wasm-plugin/`.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wasm-plugin")
        .join(name)
}

/// Raw bytes of a fixture (for hot-reload swap copies).
pub fn fixture_bytes(name: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(fixture(name))
}

/// A grant whose only scope is `data` mapped onto `dir` with the given mode.
pub fn grant_with_data(dir: &Path, mode: PreopenMode) -> PluginGrant {
    PluginGrant {
        capabilities: vec![],
        scopes: vec![ResolvedScope {
            name: "data".into(),
            host_dir: dir.into(),
            mode,
        }],
    }
}

/// A grant that supplies nothing: no capabilities, no scopes.
pub fn empty_grant() -> PluginGrant {
    PluginGrant {
        capabilities: vec![],
        scopes: vec![],
    }
}

/// Load a fixture plugin with default limits.
pub fn load(name: &str, grant: &PluginGrant) -> Result<PluginHandle, WasmError> {
    PluginHandle::load(&fixture(name), grant, PluginLimits::default())
}
