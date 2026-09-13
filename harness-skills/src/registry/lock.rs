//! The registry ledger (`<root>/.harness/registry-lock.json`, hermes §27):
//! installed pack/server provenance as a content-hash ledger. Every
//! installed file and every MCP entry is pinned, so tampering is
//! detectable ([`crate::registry::drift`]) and upgrades are diffable
//! ([`RegistryLedger::diff`]). Distinct from the T30 skills-lock file,
//! which pins whatever a directory scan finds; this ledger records what
//! the REGISTRY installed and verified.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::registry::drift::{ChangedEntry, FileChange, RegistryDiff};
use crate::registry::error::RegistryError;
use crate::registry::protocol::ReviewAcceptance;

/// Ledger schema version.
pub const LEDGER_SCHEMA: u32 = 1;

/// What kind of item an entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    /// A single pack (skill files, rules, MCP entries).
    Pack,
    /// A managed bundle unit (its plugins' files live in ONE entry).
    Bundle,
}

/// One installed item's provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Item name.
    pub name: String,
    /// Installed version.
    pub version: String,
    /// Pack or bundle.
    pub kind: ItemKind,
    /// blake3 of the pack file bytes (the pin the store verified).
    pub content_hash: String,
    /// Store id the item came from (e.g. `"local"`).
    pub source: String,
    /// Plugins a bundle entry installed.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// Review acceptance bound to the installed hash, when gated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewAcceptance>,
    /// Key → blake3 pin of every installed surface: `skills/…` and
    /// `rules/…` files (file bytes) and `mcp/<server>` entries (canonical
    /// JSON).
    pub files: BTreeMap<String, String>,
}

/// The ledger document, kept sorted by entry name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryLedger {
    /// Schema version (must be [`LEDGER_SCHEMA`]).
    pub schema: u32,
    /// Entries sorted by name.
    pub entries: Vec<LedgerEntry>,
}

impl Default for RegistryLedger {
    fn default() -> Self {
        Self {
            schema: LEDGER_SCHEMA,
            entries: Vec::new(),
        }
    }
}

impl RegistryLedger {
    /// Loads `<root>/.harness/registry-lock.json`. Missing file = empty
    /// ledger; anything unparsable is a typed [`RegistryError::LedgerCorrupt`].
    pub fn load(root: &Path) -> Result<Self, RegistryError> {
        let path = root.join(".harness").join("registry-lock.json");
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(RegistryError::io("read registry ledger", &e)),
        };
        let ledger: Self =
            serde_json::from_slice(&bytes).map_err(|e| RegistryError::LedgerCorrupt {
                detail: e.to_string(),
            })?;
        if ledger.schema != LEDGER_SCHEMA {
            return Err(RegistryError::LedgerCorrupt {
                detail: format!("unknown schema {}", ledger.schema),
            });
        }
        Ok(ledger)
    }

    /// Writes the ledger atomically (tmp + rename), entries sorted.
    pub fn save(&self, root: &Path) -> Result<(), RegistryError> {
        let mut sorted = self.clone();
        sorted.entries.sort_by(|a, b| a.name.cmp(&b.name));
        let path = root.join(".harness").join("registry-lock.json");
        let Some(parent) = path.parent() else {
            return Err(RegistryError::Io {
                detail: format!("ledger path {} has no parent", path.display()),
            });
        };
        std::fs::create_dir_all(parent)
            .map_err(|e| RegistryError::io("create .harness dir", &e))?;
        let bytes =
            serde_json::to_vec_pretty(&sorted).map_err(|e| RegistryError::LedgerCorrupt {
                detail: e.to_string(),
            })?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes).map_err(|e| RegistryError::io("write ledger tmp", &e))?;
        std::fs::rename(&tmp, &path).map_err(|e| RegistryError::io("commit ledger", &e))
    }

    /// Inserts or replaces the entry for `entry.name` (an upgrade
    /// replaces provenance wholesale — the ledger mirrors disk).
    pub fn upsert(&mut self, entry: LedgerEntry) {
        self.entries.retain(|e| e.name != entry.name);
        self.entries.push(entry);
    }

    /// Removes and returns the entry for `name`.
    pub fn remove(&mut self, name: &str) -> Option<LedgerEntry> {
        let idx = self.entries.iter().position(|e| e.name == name)?;
        Some(self.entries.remove(idx))
    }

    /// The entry for `name`.
    pub fn get(&self, name: &str) -> Option<&LedgerEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Installed item names (bundle plugins are not separate entries).
    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }

    /// Ledger-vs-ledger diff (upgrade review): every pin that moved
    /// between two ledger states, per item.
    pub fn diff(&self, new: &RegistryLedger) -> RegistryDiff {
        let mut changed = Vec::new();
        let mut names: Vec<&str> = self.entries.iter().map(|e| e.name.as_str()).collect();
        names.extend(new.entries.iter().map(|e| e.name.as_str()));
        names.sort_unstable();
        names.dedup();
        for name in names {
            let Some(old_entry) = self.get(name) else {
                continue;
            };
            let Some(new_entry) = new.get(name) else {
                continue;
            };
            let mut keys: Vec<&String> = old_entry.files.keys().collect();
            keys.extend(new_entry.files.keys());
            keys.sort_unstable();
            keys.dedup();
            let files_changed: Vec<FileChange> = keys
                .into_iter()
                .filter_map(|k| {
                    let was = old_entry.files.get(k).cloned();
                    let now = new_entry.files.get(k).cloned();
                    (was != now).then_some(FileChange {
                        key: k.clone(),
                        was,
                        now,
                    })
                })
                .collect();
            if !files_changed.is_empty() {
                changed.push(ChangedEntry {
                    name: name.to_string(),
                    files_changed,
                });
            }
        }
        RegistryDiff { changed }
    }
}
