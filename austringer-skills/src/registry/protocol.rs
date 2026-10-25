//! The pack registry protocol (todo 32): a typed index of content-addressed
//! entries plus the [`RegistryStore`] transport seam. The DEFAULT transport
//! is local files ([`crate::registry::LocalDirStore`]) — this crate contains
//! ZERO network code, so the default install path is structurally offline;
//! the daemon wires an HTTP-backed [`RegistryStore`] later behind the same
//! trait (self-hostable static index: `index.json` + `packs/<hash>.hpk`).

use serde::{Deserialize, Serialize};

use crate::registry::error::RegistryError;
use crate::registry::packfile::{validate_hash_hex, validate_item_name, validate_version};

/// Parse budget for the static index (untrusted input; bounded parse).
pub const MAX_INDEX_BYTES: usize = 1024 * 1024;
/// Index schema version understood by this build.
pub const INDEX_SCHEMA: u32 = 1;

/// blake3 of `bytes` as lowercase hex (the registry's content address).
pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// One index entry: where to find a pack, what it should hash to, and an
/// optional signature over the pack bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Item name (validated lowercase-kebab, loader-compatible).
    pub name: String,
    /// Version string (validated charset).
    pub version: String,
    /// Category (programming/domain/exemplar/bundle/…).
    pub category: String,
    /// Human summary (untrusted text, displayed only).
    pub summary: String,
    /// blake3 hex of the pack file bytes — the content address AND the pin.
    pub hash: String,
    /// Pack file size in bytes.
    pub size: u64,
    /// Signature over the pack bytes, when the registry signs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// The static index document (`index.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryIndex {
    /// Schema version (must be [`INDEX_SCHEMA`]).
    pub schema: u32,
    /// Entries, canonically sorted by name.
    pub entries: Vec<RegistryEntry>,
}

impl Default for RegistryIndex {
    fn default() -> Self {
        Self {
            schema: INDEX_SCHEMA,
            entries: Vec::new(),
        }
    }
}

impl RegistryIndex {
    /// Bounded parse with full entry validation. Registry indexes are
    /// UNTRUSTED text: names, versions, and hashes are validated, nothing
    /// is ever evaluated, and oversize input is rejected before parsing.
    pub fn parse(bytes: &[u8]) -> Result<Self, RegistryError> {
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(RegistryError::IndexTooLarge {
                size: bytes.len(),
                max: MAX_INDEX_BYTES,
            });
        }
        let mut index: Self =
            serde_json::from_slice(bytes).map_err(|e| RegistryError::IndexCorrupt {
                detail: e.to_string(),
            })?;
        if index.schema != INDEX_SCHEMA {
            return Err(RegistryError::IndexCorrupt {
                detail: format!("unknown schema {}", index.schema),
            });
        }
        index.entries.sort_by(|a, b| a.name.cmp(&b.name));
        for entry in &index.entries {
            validate_item_name(&entry.name)?;
            validate_version(&entry.version)?;
            validate_hash_hex(&entry.hash)?;
        }
        Ok(index)
    }

    /// Canonical bytes (sorted entries, stable pretty JSON for the static
    /// file humans serve/host). Deterministic for identical content.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut sorted = self.clone();
        sorted.entries.sort_by(|a, b| a.name.cmp(&b.name));
        serde_json::to_vec_pretty(&sorted).unwrap_or_default()
    }

    /// Latest entry by name.
    pub fn find(&self, name: &str) -> Option<&RegistryEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Exact name+version lookup.
    pub fn find_version(&self, name: &str, version: &str) -> Option<&RegistryEntry> {
        self.entries
            .iter()
            .find(|e| e.name == name && e.version == version)
    }

    /// Lexical search over name, category, and summary (case-insensitive
    /// containment — no ranking cloud, no telemetry).
    pub fn search(&self, query: &str) -> Vec<&RegistryEntry> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&q)
                    || e.category.to_lowercase().contains(&q)
                    || e.summary.to_lowercase().contains(&q)
            })
            .collect()
    }
}

/// The transport seam: where index bytes and content-addressed pack bytes
/// come from. The reference implementation reads a directory
/// ([`crate::registry::LocalDirStore`]); the daemon later provides an
/// HTTP-backed implementation of THIS trait over the same static-index
/// layout. Implementations must be deterministic and offline-by-default.
pub trait RegistryStore: Send + Sync {
    /// Stable identifier recorded in ledger provenance (e.g. `"local"`).
    fn id(&self) -> &str;
    /// Fetch and parse the registry index.
    fn index(&self) -> Result<RegistryIndex, RegistryError>;
    /// Fetch the raw bytes of the pack file with content address `hash`.
    fn pack_bytes(&self, hash: &str) -> Result<Vec<u8>, RegistryError>;
}

/// A trust-review ticket (hash-bound): the frontends show this to the
/// reviewer; [`ReviewAcceptance`] binds the approval to the exact hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewTicket {
    /// Item name.
    pub name: String,
    /// Item version.
    pub version: String,
    /// blake3 hex of the pack bytes this ticket describes.
    pub hash: String,
    /// Summary shown to the reviewer.
    pub summary: String,
}

impl ReviewTicket {
    /// Records a reviewer's approval of THIS ticket's hash.
    pub fn accept(&self, reviewer: &str) -> ReviewAcceptance {
        ReviewAcceptance {
            reviewer: reviewer.to_string(),
            of_hash: self.hash.clone(),
        }
    }
}

/// A recorded review acceptance (stored in the ledger entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewAcceptance {
    /// Who reviewed (identifier chosen by the frontend).
    pub reviewer: String,
    /// The hash that was reviewed — a mismatch invalidates the acceptance.
    pub of_hash: String,
}
