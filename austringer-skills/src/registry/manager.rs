//! The typed manager surface the CLI and Web pack-manager call: search,
//! list installed provenance, trust-review tickets, and remove. Install
//! lives in [`crate::registry::install`], verification in
//! [`crate::registry::drift`].

use std::collections::BTreeSet;
use std::path::Path;

use crate::registry::error::RegistryError;
use crate::registry::lock::{LedgerEntry, RegistryLedger};
use crate::registry::packfile::PackFile;
use crate::registry::protocol::{RegistryEntry, RegistryStore, ReviewTicket};
use crate::registry::writes::{prune_empty_dirs, remove_mcp_servers};

/// Namespace for the registry operations (all static — the registry is
/// stateless; state lives in the store and the ledger).
pub struct RegistryManager;

/// Per-install consent string (Gemini CLI pattern): what the item writes
/// and prompts for, generated from metadata only — bodies are untrusted
/// data and never leak into the warning.
pub(crate) fn install_warning(pack: &PackFile, plugins: &[String]) -> String {
    let mut surfaces = vec![format!(
        "{} skill file(s)",
        pack.files.len() + pack.shared.len()
    )];
    if !pack.rules.is_empty() {
        surfaces.push(format!("{} rule file(s)", pack.rules.len()));
    }
    if !pack.mcp_servers.is_empty() {
        let names = pack
            .mcp_servers
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        surfaces.push(format!("MCP server entries ({names})"));
    }
    if !pack.secret_params.is_empty() {
        let names = pack
            .secret_params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        surfaces.push(format!(
            "prompts for secrets ({names}) stored via the secret store, never in config files"
        ));
    }
    if !plugins.is_empty() {
        surfaces.push(format!("required plugins ({})", plugins.join(", ")));
    }
    format!(
        "INSTALL WARNING for '{}@{}': once active it installs {}. All content is hash-pinned and verified; review before activating.",
        pack.name,
        pack.version,
        surfaces.join("; ")
    )
}

/// Remove outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveReceipt {
    /// Removed item name.
    pub name: String,
    /// Ledger keys actually deleted from disk (shared files another
    /// installed item still claims are skipped).
    pub removed_files: Vec<String>,
}

impl RegistryManager {
    /// Lexical search over name/category/summary (offline store reads).
    pub fn search(
        store: &dyn RegistryStore,
        query: &str,
    ) -> Result<Vec<RegistryEntry>, RegistryError> {
        Ok(store.index()?.search(query).into_iter().cloned().collect())
    }

    /// Installed provenance (empty when nothing is installed).
    pub fn installed(root: &Path) -> Result<Vec<LedgerEntry>, RegistryError> {
        Ok(RegistryLedger::load(root)?.entries)
    }

    /// A trust-review ticket for `name`, bound to the entry's hash.
    pub fn review_ticket(
        store: &dyn RegistryStore,
        name: &str,
    ) -> Result<ReviewTicket, RegistryError> {
        let index = store.index()?;
        let entry = index
            .find(name)
            .ok_or_else(|| RegistryError::EntryNotFound {
                name: name.to_string(),
            })?;
        Ok(ReviewTicket {
            name: entry.name.clone(),
            version: entry.version.clone(),
            hash: entry.hash.clone(),
            summary: entry.summary.clone(),
        })
    }

    /// Removes one installed item: deletes files it solely owns, drops
    /// its MCP entries, prunes empty dirs, and rewrites the ledger.
    pub fn remove(root: &Path, name: &str) -> Result<RemoveReceipt, RegistryError> {
        let mut ledger = RegistryLedger::load(root)?;
        let entry = ledger
            .get(name)
            .cloned()
            .ok_or_else(|| RegistryError::EntryNotFound {
                name: name.to_string(),
            })?;
        let claimed_elsewhere: BTreeSet<&String> = ledger
            .entries
            .iter()
            .filter(|e| e.name != name)
            .flat_map(|e| e.files.keys())
            .collect();
        let austringer_root = root.join(".austringer");
        let mut removed = Vec::new();
        let mut mcp_servers = Vec::new();
        for key in entry.files.keys() {
            if claimed_elsewhere.contains(key) {
                continue;
            }
            if let Some(server) = key.strip_prefix("mcp/") {
                mcp_servers.push(server.to_string());
                removed.push(key.clone());
                continue;
            }
            let path = austringer_root.join(key);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    removed.push(key.clone());
                    prune_empty_dirs(&path, &austringer_root);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(RegistryError::io("remove installed file", &e)),
            }
        }
        if !mcp_servers.is_empty() {
            remove_mcp_servers(root, &mcp_servers)?;
        }
        ledger.remove(name);
        ledger.save(root)?;
        Ok(RemoveReceipt {
            name: entry.name,
            removed_files: removed,
        })
    }
}
