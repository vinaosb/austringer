//! The pack file format (`.hpk`): a SINGLE JSON document carrying every
//! file a marketplace item needs. Items are DECLARATIVE DATA ONLY (Roo
//! §10): files, shared partials, rule texts, MCP server entries, and
//! prompted-secret declarations — there is no code path that interprets
//! an item's content at install time. MCP entries keep their
//! `{{secret:NAME}}` placeholders; values resolve through the secret
//! source seam at runtime and NEVER enter config files (T29 discipline).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::registry::bundles::BundleManifest;
use crate::registry::error::RegistryError;

/// Parse budget for one pack file (untrusted input, bounded before parse).
pub const MAX_PACK_BYTES: usize = 8 * 1024 * 1024;
/// Pack document schema understood by this build.
pub const PACK_SCHEMA: u32 = 1;

/// A prompted install-time secret declaration. `name` is resolved through
/// the [`crate::registry::SecretSource`] seam; the value never persists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretParam {
    /// Secret name (UPPER_SNAKE).
    pub name: String,
    /// Plain-language prompt shown at install.
    pub prompt: String,
}

/// A registry pack document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackFile {
    /// Schema version (must be [`PACK_SCHEMA`]).
    pub schema: u32,
    /// Item name (loader-safe lowercase-kebab).
    pub name: String,
    /// Item version.
    pub version: String,
    /// Human summary (untrusted text, display only).
    #[serde(default)]
    pub summary: String,
    /// Skill/files payload: relative path under `skills/<name>/`.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// `_shared` partials keyed by file stem (self-contained per pack).
    #[serde(default)]
    pub shared: BTreeMap<String, String>,
    /// Rule documents keyed by file name, written under `.harness/rules/`.
    #[serde(default)]
    pub rules: BTreeMap<String, String>,
    /// MCP server entries (placeholder-preserving JSON).
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, serde_json::Value>,
    /// Secrets prompted at install.
    #[serde(default)]
    pub secret_params: Vec<SecretParam>,
    /// Managed bundle manifest, when this pack IS a bundle unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<BundleManifest>,
}

impl PackFile {
    /// An empty pack (marketplace items are built programmatically, then
    /// published via [`crate::registry::add_pack`]).
    pub fn empty(name: &str, version: &str) -> Self {
        Self {
            schema: PACK_SCHEMA,
            name: name.to_string(),
            version: version.to_string(),
            summary: String::new(),
            files: BTreeMap::new(),
            shared: BTreeMap::new(),
            rules: BTreeMap::new(),
            mcp_servers: BTreeMap::new(),
            secret_params: Vec::new(),
            bundle: None,
        }
    }

    /// Size gate: enforced before any parse of pack bytes.
    pub fn check_size(size: usize) -> Result<(), RegistryError> {
        if size > MAX_PACK_BYTES {
            return Err(RegistryError::PackTooLarge {
                size,
                max: MAX_PACK_BYTES,
            });
        }
        Ok(())
    }

    /// Canonical bytes (BTreeMap-sorted keys; byte-stable across builds —
    /// the content address depends on this determinism).
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Bounded parse with full structural validation: names, versions,
    /// pins, paths, secret names, and the embedded bundle manifest.
    pub fn parse(bytes: &[u8]) -> Result<Self, RegistryError> {
        Self::check_size(bytes.len())?;
        let pack: Self = serde_json::from_slice(bytes).map_err(|e| RegistryError::PackCorrupt {
            detail: e.to_string(),
        })?;
        if pack.schema != PACK_SCHEMA {
            return Err(RegistryError::PackCorrupt {
                detail: format!("unknown schema {}", pack.schema),
            });
        }
        pack.validate()?;
        Ok(pack)
    }

    /// Validates all untrusted text fields (used at parse AND publish).
    pub fn validate(&self) -> Result<(), RegistryError> {
        validate_item_name(&self.name)?;
        validate_version(&self.version)?;
        for path in self.files.keys() {
            if !Self::validate_relpath(path) {
                return Err(RegistryError::InvalidPackPath { path: path.clone() });
            }
        }
        for stem in self.shared.keys() {
            validate_item_name(stem)?;
        }
        for rule in self.rules.keys() {
            if !Self::validate_relpath(rule) {
                return Err(RegistryError::InvalidPackPath { path: rule.clone() });
            }
        }
        for server in self.mcp_servers.keys() {
            validate_item_name(server)?;
        }
        for param in &self.secret_params {
            validate_secret_name(&param.name)?;
        }
        if let Some(bundle) = &self.bundle {
            bundle.validate()?;
            if bundle.plugins.iter().any(|p| p.name == self.name) {
                return Err(RegistryError::NestedBundle {
                    name: self.name.clone(),
                });
            }
        }
        Ok(())
    }

    /// A safe relative pack path: no traversal, no absolute paths, no
    /// Windows separators, no control characters, no empty segments.
    pub fn validate_relpath(path: &str) -> bool {
        if path.is_empty() || path.len() > 256 {
            return false;
        }
        if path.contains('\\') || path.starts_with('/') || path.contains('\0') {
            return false;
        }
        if path.chars().any(|c| c.is_control()) {
            return false;
        }
        let mut segments = path.split('/');
        segments.all(|s| !s.is_empty() && s != "." && s != "..")
    }
}

/// Loader-safe item name: `[a-z0-9-]`, 1–64 chars, no edge hyphens.
pub fn validate_item_name(name: &str) -> Result<(), RegistryError> {
    let ok = (1..=64).contains(&name.len())
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(RegistryError::InvalidName {
            name: name.to_string(),
        })
    }
}

/// Secret name: `UPPER_SNAKE` (`[A-Z][A-Z0-9_]*`, 1–64 chars).
pub fn validate_secret_name(name: &str) -> Result<(), RegistryError> {
    let mut chars = name.chars();
    let ok = (1..=64).contains(&name.len())
        && chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(RegistryError::InvalidName {
            name: name.to_string(),
        })
    }
}

/// Version charset: semver-ish `[0-9A-Za-z.+-]`, 1–64 chars.
pub fn validate_version(version: &str) -> Result<(), RegistryError> {
    let ok = (1..=64).contains(&version.len())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'));
    if ok {
        Ok(())
    } else {
        Err(RegistryError::InvalidVersion {
            version: version.to_string(),
        })
    }
}

/// A 64-char lowercase-hex blake3 content pin.
pub fn validate_hash_hex(hash: &str) -> Result<(), RegistryError> {
    if hash.len() == 64
        && hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(RegistryError::InvalidPin {
            pin: hash.to_string(),
        })
    }
}

/// Alias of [`validate_hash_hex`] for bundle pin fields.
pub fn validate_pin(pin: &str) -> Result<(), RegistryError> {
    validate_hash_hex(pin)
}

/// A governance/manifest glob: `[a-z0-9-*._]`, 1–128 chars, no `/`, no `..`.
pub fn validate_glob(glob: &str) -> Result<(), RegistryError> {
    let ok = (1..=128).contains(&glob.len())
        && !glob.contains("..")
        && glob.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '*' | '-' | '.' | '_')
        });
    if ok {
        Ok(())
    } else {
        Err(RegistryError::InvalidName {
            name: glob.to_string(),
        })
    }
}
