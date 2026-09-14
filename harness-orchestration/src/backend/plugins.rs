//! Per-harness plugin managers (Warp, SS25): marketplace plugins
//! installed and version-checked per external CLI agent, pinned by
//! content hash.

use std::collections::BTreeMap;

use super::install::verify_sha256;

/// Duplicate plugin registration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("duplicate plugin `{0}`")]
pub struct DuplicatePlugin(String);

/// One installed plugin record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRecord {
    /// Plugin name.
    pub name: String,
    /// Installed version.
    pub version: String,
    /// Pinned content hash (SHA-256).
    pub sha256: String,
}

/// Numeric-aware version comparison: `a > b` segment by segment, longer
/// wins on equal prefixes.
pub fn version_gt(a: &str, b: &str) -> bool {
    let left: Vec<u64> = a
        .split('.')
        .map(|s| s.trim().parse::<u64>().unwrap_or(0))
        .collect();
    let right: Vec<u64> = b
        .split('.')
        .map(|s| s.trim().parse::<u64>().unwrap_or(0))
        .collect();
    for (x, y) in left.iter().zip(right.iter()) {
        if x != y {
            return x > y;
        }
    }
    left.len() > right.len()
}

/// The plugin set for ONE external harness (each backend manages its own
/// plugins — never a shared pool).
#[derive(Debug, Clone)]
pub struct HarnessPlugins {
    backend: super::BackendKind,
    plugins: BTreeMap<String, PluginRecord>,
}

impl HarnessPlugins {
    /// An empty plugin set for a backend.
    pub fn new(backend: super::BackendKind) -> Self {
        Self {
            backend,
            plugins: BTreeMap::new(),
        }
    }

    /// Which backend this set belongs to.
    pub fn backend(&self) -> super::BackendKind {
        self.backend
    }

    /// Registers (or replaces) a plugin record.
    pub fn register(&mut self, record: PluginRecord) -> Result<(), DuplicatePlugin> {
        self.plugins.insert(record.name.clone(), record);
        Ok(())
    }

    /// Whether the marketplace version outruns the installed record.
    pub fn needs_update(&self, name: &str, marketplace_version: &str) -> bool {
        self.plugins
            .get(name)
            .is_some_and(|record| version_gt(marketplace_version, &record.version))
    }

    /// Verifies plugin bytes against a pinned SHA-256.
    pub fn verify_plugin(bytes: &[u8], sha256: &str) -> bool {
        verify_sha256(bytes, sha256)
    }
}
