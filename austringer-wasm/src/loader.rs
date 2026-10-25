//! The load pipeline: bytes -> compiled module -> manifest -> interface
//! negotiation -> capability check -> preopen check -> [`LoadedPlugin`].
//! Every rejection is typed ([`crate::error::WasmError`]); nothing loads
//! half-way.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::abi::{self, Interface};
use crate::error::WasmError;
use crate::hostfns::PluginGrant;
use crate::limits::PluginLimits;
use crate::manifest::{self, PluginManifest};
use crate::runtime::{self, WasmEngine};
/// Cheap file identity for the hot-reload watcher: length + mtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFingerprint {
    /// File length in bytes.
    pub len: u64,
    /// Last modification time.
    pub mtime: SystemTime,
}

/// Read a file's current fingerprint.
pub fn fingerprint(path: &Path) -> Result<FileFingerprint, WasmError> {
    let meta = std::fs::metadata(path).map_err(|e| WasmError::HostIo {
        detail: format!("{}: {e}", path.display()),
    })?;
    Ok(FileFingerprint {
        len: meta.len(),
        mtime: meta.modified().map_err(|e| WasmError::HostIo {
            detail: e.to_string(),
        })?,
    })
}

/// A fully validated, ready-to-call plugin.
pub struct LoadedPlugin {
    /// The compiled module (cheap to clone via Arc at the handle layer).
    pub module: wasmtime::Module,
    /// The validated manifest.
    pub manifest: PluginManifest,
    /// The negotiated interface dispatch arm.
    pub interface: Interface,
    /// Where the plugin was loaded from (hot reload re-reads this path).
    pub path: PathBuf,
    /// Identity of the loaded bytes.
    pub fingerprint: FileFingerprint,
}

/// The full load pipeline. `stem` is the file stem used in pre-manifest
/// errors (the manifest name is not known yet); post-manifest errors use
/// the declared plugin name.
pub fn load(
    engine: &WasmEngine,
    stem: &str,
    path: &Path,
    grant: &PluginGrant,
    limits: &PluginLimits,
) -> Result<LoadedPlugin, WasmError> {
    let bytes = std::fs::read(path).map_err(|e| WasmError::HostIo {
        detail: format!("{}: {e}", path.display()),
    })?;
    let module = engine.compile(stem, &bytes)?;

    let manifest_bytes = read_manifest_region(&module, limits)?;
    if manifest_bytes.is_empty() {
        return Err(WasmError::ManifestMissing {
            plugin: stem.into(),
        });
    }
    let manifest = manifest::parse(&manifest_bytes)?;

    let interface = abi::negotiate(manifest.interface).map_err(|e| match e {
        abi::InterfaceError::TooOld { declared, min } => WasmError::InterfaceTooOld {
            plugin: manifest.name.clone(),
            declared,
            min,
        },
        abi::InterfaceError::TooNew { declared, max } => WasmError::InterfaceTooNew {
            plugin: manifest.name.clone(),
            declared,
            max,
        },
    })?;

    for capability in &manifest.capabilities {
        if !grant.capabilities.contains(capability) {
            return Err(WasmError::CapabilityNotGranted {
                plugin: manifest.name.clone(),
                capability: capability.to_string(),
            });
        }
    }

    for preopen in &manifest.preopens {
        let Some(scope) = grant.scopes.iter().find(|s| s.name == preopen.name) else {
            return Err(WasmError::PreopenUnsatisfied {
                plugin: manifest.name.clone(),
                scope: preopen.name.clone(),
            });
        };
        if preopen.mode == crate::manifest::PreopenMode::ReadWrite
            && scope.mode == crate::manifest::PreopenMode::ReadOnly
        {
            return Err(WasmError::PreopenModeDenied {
                plugin: manifest.name.clone(),
                scope: preopen.name.clone(),
            });
        }
    }

    Ok(LoadedPlugin {
        module,
        manifest,
        interface,
        path: path.to_path_buf(),
        fingerprint: fingerprint(path)?,
    })
}

/// Read the NUL-terminated manifest region out of a freshly instantiated
/// module, under the same limiter + fuel regime as real calls (a hostile
/// start function or oversized initial memory dies here, bounded).
fn read_manifest_region(
    module: &wasmtime::Module,
    limits: &PluginLimits,
) -> Result<Vec<u8>, WasmError> {
    let (mut store, linker) = runtime::fresh_store(module, limits)?;
    let (_, memory) =
        runtime::instantiate(&mut store, &linker, module, "manifest-probe", Vec::new())?;
    let mut raw = vec![0_u8; abi::MANIFEST_CAPACITY];
    memory
        .read(&mut store, abi::MANIFEST_OFFSET, &mut raw)
        .map_err(|e| WasmError::InstantiateFailed {
            plugin: "manifest-probe".into(),
            detail: format!("manifest region unreadable: {e}"),
        })?;
    let end = raw.iter().position(|b| *b == 0).ok_or(WasmError::Manifest(
        manifest::ManifestError::TooLarge {
            capacity: abi::MANIFEST_CAPACITY,
        },
    ))?;
    Ok(raw[..end].to_vec())
}
