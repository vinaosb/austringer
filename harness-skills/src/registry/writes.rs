//! Staged install writes: ALL surface changes (skill files, shared
//! partials, rule files, MCP entries) are collected into a [`WriteSet`]
//! BEFORE anything touches disk, and committed in one pass — a bundle
//! whose plugins fail pin checks writes nothing (fail-closed install).
//! MCP entries keep `{{secret:…}}` placeholders; secret values never
//! enter config files (T29 discipline — the daemon substitutes at
//! runtime from the real secret store).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::registry::drift::canonical_json;
use crate::registry::error::RegistryError;
use crate::registry::packfile::PackFile;
use crate::registry::protocol::blake3_hex;

/// Every surface one install (or one bundle unit) will write, keyed by
/// ledger key: `skills/…`, `rules/…` file contents and `mcp/…` entries.
#[derive(Debug, Default)]
pub(crate) struct WriteSet {
    files: BTreeMap<String, String>,
    mcp: BTreeMap<String, Value>,
}

impl WriteSet {
    /// Stages one pack's declarative payload (files land under
    /// `skills/<name>/`, shared partials under `skills/_shared/`).
    pub(crate) fn add_pack(&mut self, pack: &PackFile) -> Result<(), RegistryError> {
        for (rel, content) in &pack.files {
            self.files
                .insert(format!("skills/{}/{}", pack.name, rel), content.clone());
        }
        for (stem, content) in &pack.shared {
            self.files
                .insert(format!("skills/_shared/{stem}.md"), content.clone());
        }
        for (rule, content) in &pack.rules {
            if !PackFile::validate_relpath(rule) {
                return Err(RegistryError::InvalidPackPath { path: rule.clone() });
            }
            self.files.insert(format!("rules/{rule}"), content.clone());
        }
        for (server, value) in &pack.mcp_servers {
            self.mcp
                .insert(format!("mcp/{server}"), disabled_declaration(value));
        }
        Ok(())
    }

    /// Commits everything and returns the ledger pins (key → blake3 of
    /// written bytes / canonical MCP JSON).
    pub(crate) fn commit(self, root: &Path) -> Result<BTreeMap<String, String>, RegistryError> {
        let mut pins = BTreeMap::new();
        for (key, content) in &self.files {
            let path = root.join(".harness").join(key);
            write_file(&path, content)?;
            pins.insert(key.clone(), blake3_hex(content.as_bytes()));
        }
        if !self.mcp.is_empty() {
            merge_mcp(root, &self.mcp)?;
            for (key, value) in &self.mcp {
                pins.insert(key.clone(), blake3_hex(canonical_json(value).as_bytes()));
            }
        }
        Ok(pins)
    }
}

fn write_file(path: &Path, content: &str) -> Result<(), RegistryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| RegistryError::io("create install dir", &e))?;
    }
    std::fs::write(path, content).map_err(|e| RegistryError::io("write installed file", &e))
}

/// Copies one pack MCP entry into its DISABLED declaration form:
/// `trusted_autostart` is forced `false` whatever the pack claims (a
/// pack may never self-enable an executable-manifest entry — the only
/// enable path is a separate explicit consent step outside the install
/// pipeline). Non-object values pass through untouched for the
/// scanner's per-entry diagnostics to reject.
fn disabled_declaration(value: &Value) -> Value {
    let mut declared = value.clone();
    if let Some(fields) = declared.as_object_mut() {
        fields.insert("trusted_autostart".to_owned(), Value::Bool(false));
    }
    declared
}

/// Merges server entries into `<root>/.harness/mcp.json` (the cross-tool
/// `mcpServers` convention), preserving existing entries.
fn merge_mcp(root: &Path, mcp: &BTreeMap<String, Value>) -> Result<(), RegistryError> {
    let path = root.join(".harness").join("mcp.json");
    let mut doc = match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice::<Value>(&bytes).map_err(|e| RegistryError::LedgerCorrupt {
                detail: format!("existing mcp.json: {e}"),
            })?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({"mcpServers": {}}),
        Err(e) => return Err(RegistryError::io("read mcp.json", &e)),
    };
    if doc.get("mcpServers").map(Value::is_object) != Some(true) {
        doc["mcpServers"] = json!({});
    }
    if let Some(servers) = doc.get_mut("mcpServers").and_then(Value::as_object_mut) {
        for (key, value) in mcp {
            if let Some(server) = key.strip_prefix("mcp/") {
                servers.insert(server.to_string(), value.clone());
            }
        }
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|e| RegistryError::Io {
        detail: format!("serialize mcp.json: {e}"),
    })?;
    write_file(&path, &text)
}

/// Removes servers from `mcp.json`, deleting the file when it empties.
pub(crate) fn remove_mcp_servers(root: &Path, servers: &[String]) -> Result<(), RegistryError> {
    let path = root.join(".harness").join("mcp.json");
    let bytes = std::fs::read(&path).map_err(|e| RegistryError::io("read mcp.json", &e))?;
    let mut doc: Value =
        serde_json::from_slice(&bytes).map_err(|e| RegistryError::LedgerCorrupt {
            detail: format!("mcp.json: {e}"),
        })?;
    let empty = {
        let Some(map) = doc.get_mut("mcpServers").and_then(Value::as_object_mut) else {
            return Ok(());
        };
        for server in servers {
            map.remove(server);
        }
        map.is_empty()
    };
    if empty {
        std::fs::remove_file(&path).map_err(|e| RegistryError::io("remove empty mcp.json", &e))
    } else {
        let text = serde_json::to_string_pretty(&doc).map_err(|e| RegistryError::Io {
            detail: format!("serialize mcp.json: {e}"),
        })?;
        write_file(&path, &text)
    }
}

/// Prunes directories left empty by file removal, walking up from
/// `file` toward the `.harness` root (never removing the root itself).
pub(crate) fn prune_empty_dirs(file: &Path, stop: &Path) {
    let mut dir: Option<PathBuf> = file.parent().map(Path::to_path_buf);
    while let Some(current) = dir {
        if current == stop || std::fs::remove_dir(&current).is_err() {
            return;
        }
        dir = current.parent().map(Path::to_path_buf);
    }
}
