//! The install pipeline (todo 32): governance → review gate → signature
//! presence → fetch + PIN/HASH VERIFY → signature verify → bounded pack
//! parse → secret-resolution gate → bundle plugin resolution (pins
//! checked against the index BEFORE any byte is written) → one committed
//! [`WriteSet`] → ledger upsert. Every rejection is typed and leaves
//! NOTHING on disk.

use std::path::{Path, PathBuf};

use crate::registry::bundles::check_bundle;
use crate::registry::error::RegistryError;
use crate::registry::governance::GovernanceStack;
use crate::registry::lock::{ItemKind, LedgerEntry, RegistryLedger};
use crate::registry::manager::RegistryManager;
use crate::registry::packfile::PackFile;
use crate::registry::protocol::{RegistryEntry, RegistryStore, ReviewAcceptance, blake3_hex};
use crate::registry::signatures::SignatureVerifier;
use crate::registry::writes::WriteSet;

/// Install scope: project-local or user-global (same `.harness` layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// `<root>/.harness` inside one project.
    Project(PathBuf),
    /// `<root>/.harness` in the user's home.
    Global(PathBuf),
}

impl Scope {
    /// Project-scoped install root.
    pub fn project(root: PathBuf) -> Self {
        Self::Project(root)
    }

    /// User-global install root.
    pub fn global(root: PathBuf) -> Self {
        Self::Global(root)
    }

    /// The directory holding `.harness`.
    pub fn root(&self) -> &Path {
        match self {
            Self::Project(p) | Self::Global(p) => p,
        }
    }
}

/// Where install-time secret prompts resolve from (the SecretStore
/// seam). Values are checked for RESOLVABILITY only — they never enter
/// config files and never persist in the ledger.
pub trait SecretSource: Send + Sync {
    /// The secret's value, or None when unresolvable.
    fn resolve(&self, name: &str) -> Option<String>;
}

/// In-memory [`SecretSource`] (tests, CLI prompts, daemon keyring shim).
#[derive(Debug, Clone)]
pub struct MapSecretSource(std::collections::BTreeMap<String, String>);

impl MapSecretSource {
    /// Wraps a name→value map.
    pub fn new(map: std::collections::BTreeMap<String, String>) -> Self {
        Self(map)
    }
}

impl SecretSource for MapSecretSource {
    fn resolve(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

/// Install knobs. Defaults: no governance layers, signatures verified
/// when present, no review gate.
#[derive(Clone, Default)]
pub struct InstallOptions<'a> {
    /// Plugin governance stack (higher authority wins; forbid-`*` =
    /// lockdown).
    pub governance: GovernanceStack,
    /// Signature verifier for signed entries (fail-closed when missing).
    pub verifier: Option<&'a dyn SignatureVerifier>,
    /// Refuse unsigned entries entirely.
    pub require_signature: bool,
    /// Require a hash-bound review acceptance.
    pub require_review: bool,
    /// The reviewer's acceptance (must bind to the installed hash).
    pub acceptance: Option<&'a ReviewAcceptance>,
}

/// Install outcome: what landed, pinned to what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReceipt {
    /// Item name.
    pub name: String,
    /// Version installed.
    pub version: String,
    /// The verified content hash (the pin).
    pub hash: String,
    /// Ledger keys written, sorted.
    pub files: Vec<String>,
    /// Plugins a bundle unit installed.
    pub plugins_installed: Vec<String>,
    /// Optional plugins the frontends may offer next.
    pub optional_available: Vec<String>,
    /// Plain-language per-install consent warning (Gemini CLI pattern).
    pub install_warning: String,
}

impl RegistryManager {
    /// Installs `name` from `store` into `scope`. The typed API the CLI
    /// and Web manager call; the default store is local files (offline).
    pub fn install(
        store: &dyn RegistryStore,
        name: &str,
        scope: &Scope,
        options: &InstallOptions<'_>,
        secrets: &dyn SecretSource,
    ) -> Result<InstallReceipt, RegistryError> {
        let index = store.index()?;
        let entry = index
            .find(name)
            .cloned()
            .ok_or_else(|| RegistryError::EntryNotFound {
                name: name.to_string(),
            })?;
        options.governance.permit(name)?;
        if options.require_review {
            let acceptance = options
                .acceptance
                .ok_or_else(|| RegistryError::Unreviewed {
                    name: entry.name.clone(),
                })?;
            if acceptance.of_hash != entry.hash {
                return Err(RegistryError::Unreviewed {
                    name: entry.name.clone(),
                });
            }
        }
        if entry.signature.is_none() && options.require_signature {
            return Err(RegistryError::SignatureMissing { name: entry.name });
        }
        if entry.signature.is_some() && options.verifier.is_none() {
            return Err(RegistryError::SignatureVerifierMissing { name: entry.name });
        }
        let bytes = fetch_verified(store, &entry)?;
        verify_signature(&entry, &bytes, options)?;
        let pack = PackFile::parse(&bytes)?;
        for param in &pack.secret_params {
            if secrets.resolve(&param.name).is_none() {
                return Err(RegistryError::MissingSecret {
                    name: param.name.clone(),
                });
            }
        }

        let mut ledger = RegistryLedger::load(scope.root())?;
        let mut writes = WriteSet::default();
        writes.add_pack(&pack)?;
        let mut plugins_installed = Vec::new();
        let mut optional_available = Vec::new();
        if let Some(bundle) = &pack.bundle {
            let check = check_bundle(bundle, &ledger.names(), &options.governance)?;
            for secret in &bundle.secret_refs {
                if secrets.resolve(secret).is_none() {
                    return Err(RegistryError::MissingSecret {
                        name: secret.clone(),
                    });
                }
            }
            for plugin in &check.to_install {
                let plugin_pack = resolve_plugin(
                    store,
                    &index,
                    plugin.name.as_str(),
                    plugin.pin.as_str(),
                    options,
                )?;
                writes.add_pack(&plugin_pack)?;
                plugins_installed.push(plugin.name.clone());
            }
            optional_available = check.optional_available;
        }

        let pins = writes.commit(scope.root())?;
        let kind = if pack.bundle.is_some() {
            ItemKind::Bundle
        } else {
            ItemKind::Pack
        };
        ledger.upsert(LedgerEntry {
            name: entry.name.clone(),
            version: entry.version.clone(),
            kind,
            content_hash: entry.hash.clone(),
            source: store.id().to_string(),
            plugins: plugins_installed.clone(),
            review: options.acceptance.cloned(),
            files: pins,
        });
        ledger.save(scope.root())?;
        Ok(InstallReceipt {
            install_warning: crate::registry::manager::install_warning(&pack, &plugins_installed),
            name: entry.name,
            version: entry.version,
            hash: entry.hash,
            files: ledger
                .get(name)
                .map(|e| e.files.keys().cloned().collect())
                .unwrap_or_default(),
            plugins_installed,
            optional_available,
        })
    }
}

fn resolve_plugin(
    store: &dyn RegistryStore,
    index: &crate::registry::protocol::RegistryIndex,
    name: &str,
    pin: &str,
    options: &InstallOptions<'_>,
) -> Result<PackFile, RegistryError> {
    let entry = index
        .find(name)
        .cloned()
        .ok_or_else(|| RegistryError::MissingRequiredPlugin {
            plugin: name.to_string(),
        })?;
    if entry.hash != pin {
        return Err(RegistryError::PinMismatch {
            plugin: name.to_string(),
            pinned: pin.to_string(),
            actual: entry.hash,
        });
    }
    if entry.signature.is_some() && options.verifier.is_none() {
        return Err(RegistryError::SignatureVerifierMissing { name: entry.name });
    }
    let bytes = fetch_verified(store, &entry)?;
    verify_signature(&entry, &bytes, options)?;
    let pack = PackFile::parse(&bytes)?;
    if pack.bundle.is_some() {
        return Err(RegistryError::NestedBundle {
            name: name.to_string(),
        });
    }
    Ok(pack)
}

fn fetch_verified(
    store: &dyn RegistryStore,
    entry: &RegistryEntry,
) -> Result<Vec<u8>, RegistryError> {
    let bytes = store.pack_bytes(&entry.hash)?;
    let actual = blake3_hex(&bytes);
    if actual != entry.hash {
        return Err(RegistryError::HashMismatch {
            name: entry.name.clone(),
            expected: entry.hash.clone(),
            actual,
        });
    }
    Ok(bytes)
}

fn verify_signature(
    entry: &RegistryEntry,
    bytes: &[u8],
    options: &InstallOptions<'_>,
) -> Result<(), RegistryError> {
    if let (Some(sig), Some(verifier)) = (&entry.signature, options.verifier)
        && !verifier.verify(bytes, sig)
    {
        return Err(RegistryError::SignatureInvalid {
            name: entry.name.clone(),
        });
    }
    Ok(())
}
