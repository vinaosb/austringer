//! Ledger-vs-disk drift (stale-state channel): every installed surface
//! pinned in the registry ledger is re-hashed and compared, and files
//! that appeared behind the ledger's back inside owned skill dirs are
//! reported. [`RegistryManager::verify`] returns typed drift;
//! [`RegistryManager::diff_disk`] renders the same scan as was/now diff
//! rows (the upgrade-review view).

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::registry::error::RegistryError;
use crate::registry::lock::RegistryLedger;
use crate::registry::manager::RegistryManager;
use crate::registry::protocol::blake3_hex;

/// One typed drift finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Drift {
    /// A pinned file or MCP entry vanished from disk.
    Missing {
        /// Owning ledger entry name.
        name: String,
        /// Ledger key (`skills/…`, `rules/…`, `mcp/…`).
        key: String,
    },
    /// On-disk content no longer hashes to the ledger pin.
    Modified {
        /// Owning ledger entry name.
        name: String,
        /// Ledger key.
        key: String,
        /// The ledger pin.
        expected: String,
        /// Hash of what is on disk.
        actual: String,
    },
    /// A file inside an owned skill dir that no install recorded.
    Unexpected {
        /// Owning ledger entry name.
        name: String,
        /// On-disk key (`skills/…`).
        key: String,
    },
}

/// One file-level was/now row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    /// Ledger key.
    pub key: String,
    /// Pin before (None = not previously pinned).
    pub was: Option<String>,
    /// Hash now (None = gone from disk).
    pub now: Option<String>,
}

/// Per-item changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedEntry {
    /// Ledger entry name.
    pub name: String,
    /// Changed keys.
    pub files_changed: Vec<FileChange>,
}

/// A diff result: only items with at least one change appear.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryDiff {
    /// Changed items.
    pub changed: Vec<ChangedEntry>,
}

impl RegistryManager {
    /// Verifies every ledger entry against disk.
    pub fn verify(root: &Path) -> Result<Vec<Drift>, RegistryError> {
        let ledger = RegistryLedger::load(root)?;
        let mut drift = Vec::new();
        for entry in &ledger.entries {
            drift.extend(entry_drift(root, entry.name.as_str(), entry)?);
        }
        Ok(drift)
    }

    /// Verifies one entry against disk. Empty = clean.
    pub fn verify_entry(root: &Path, name: &str) -> Result<Vec<Drift>, RegistryError> {
        let ledger = RegistryLedger::load(root)?;
        let entry = ledger
            .get(name)
            .ok_or_else(|| RegistryError::EntryNotFound {
                name: name.to_string(),
            })?;
        entry_drift(root, name, entry)
    }

    /// Ledger-vs-disk diff (was = pin, now = current hash, None = absent).
    pub fn diff_disk(root: &Path) -> Result<RegistryDiff, RegistryError> {
        let ledger = RegistryLedger::load(root)?;
        let mut changed = Vec::new();
        for entry in &ledger.entries {
            let mut rows = Vec::new();
            for drift in entry_drift(root, entry.name.as_str(), entry)? {
                let (key, was, now) = match drift {
                    Drift::Missing { key, .. } => {
                        let was = entry.files.get(&key).cloned();
                        (key, was, None)
                    }
                    Drift::Modified {
                        key,
                        expected,
                        actual,
                        ..
                    } => (key, Some(expected), Some(actual)),
                    Drift::Unexpected { key, .. } => (key.clone(), None, None),
                };
                let now = match now {
                    Some(h) => Some(h),
                    None => disk_hash(root, &key),
                };
                rows.push(FileChange { key, was, now });
            }
            if !rows.is_empty() {
                changed.push(ChangedEntry {
                    name: entry.name.clone(),
                    files_changed: rows,
                });
            }
        }
        Ok(RegistryDiff { changed })
    }
}

fn entry_drift(
    root: &Path,
    name: &str,
    entry: &crate::registry::lock::LedgerEntry,
) -> Result<Vec<Drift>, RegistryError> {
    let mut drift = Vec::new();
    for (key, pin) in &entry.files {
        if let Some(server) = key.strip_prefix("mcp/") {
            match mcp_server_hash(root, server)? {
                None => drift.push(Drift::Missing {
                    name: name.to_string(),
                    key: key.clone(),
                }),
                Some(actual) if &actual != pin => drift.push(Drift::Modified {
                    name: name.to_string(),
                    key: key.clone(),
                    expected: pin.clone(),
                    actual,
                }),
                Some(_) => {}
            }
            continue;
        }
        match std::fs::read(root.join(".austringer").join(key)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => drift.push(Drift::Missing {
                name: name.to_string(),
                key: key.clone(),
            }),
            Err(e) => return Err(RegistryError::io("verify file", &e)),
            Ok(bytes) => {
                let actual = blake3_hex(&bytes);
                if &actual != pin {
                    drift.push(Drift::Modified {
                        name: name.to_string(),
                        key: key.clone(),
                        expected: pin.clone(),
                        actual,
                    });
                }
            }
        }
    }
    drift.extend(unexpected_files(root, name, entry));
    Ok(drift)
}

/// Hash of one server entry inside `.austringer/mcp.json` (None when the
/// file or the entry is gone).
fn mcp_server_hash(root: &Path, server: &str) -> Result<Option<String>, RegistryError> {
    let bytes = match std::fs::read(root.join(".austringer").join("mcp.json")) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(RegistryError::io("read mcp.json", &e)),
    };
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| RegistryError::LedgerCorrupt {
            detail: format!("mcp.json: {e}"),
        })?;
    let entry = value.get("mcpServers").and_then(|s| s.get(server)).cloned();
    Ok(entry.map(|v| blake3_hex(canonical_json(&v).as_bytes())))
}

fn disk_hash(root: &Path, key: &str) -> Option<String> {
    if key.starts_with("mcp/") {
        return None;
    }
    std::fs::read(root.join(".austringer").join(key))
        .ok()
        .map(|b| blake3_hex(&b))
}

fn unexpected_files(
    root: &Path,
    name: &str,
    entry: &crate::registry::lock::LedgerEntry,
) -> Vec<Drift> {
    let owned: BTreeSet<&str> = entry
        .files
        .keys()
        .filter_map(|k| k.strip_prefix("skills/"))
        .filter_map(|rest| rest.split('/').next())
        .filter(|dir| *dir != "_shared")
        .collect();
    let mut drift = Vec::new();
    for dir in owned {
        let base = root.join(".austringer").join("skills").join(dir);
        for rel in walk_keys(&base, "", 0) {
            let key = format!("skills/{dir}/{rel}");
            if !entry.files.contains_key(&key) {
                drift.push(Drift::Unexpected {
                    name: name.to_string(),
                    key,
                });
            }
        }
    }
    drift
}

fn walk_keys(dir: &Path, prefix: &str, depth: u32) -> Vec<String> {
    if depth > 16 {
        return Vec::new();
    }
    let mut keys = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return keys;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let rel = if prefix.is_empty() {
            file
        } else {
            format!("{prefix}/{file}")
        };
        if path.is_dir() {
            keys.extend(walk_keys(&path, &rel, depth + 1));
        } else {
            keys.push(rel);
        }
    }
    keys
}

/// Canonical JSON text (sorted object keys; serde_json Map is BTreeMap
/// unless `preserve_order` — workspace-forbidden).
pub(crate) fn canonical_json(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
