//! The reference registry server (todo 32): a self-hostable STATIC index
//! — `index.json` plus content-addressed `packs/<blake3>.hpk` files in one
//! directory. [`build_registry_from_packs_dir`] produces it from the
//! shipped T33 packs tree (or any tree with the same manifest shape);
//! [`LocalDirStore`] is the default, OFFLINE transport over it. HTTP
//! serving is daemon scope: an HTTP-backed [`RegistryStore`] later reads
//! the exact same layout, so this crate contains zero network code.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::registry::bundles::BundleManifest;
use crate::registry::error::RegistryError;
use crate::registry::packfile::{PackFile, validate_item_name};
use crate::registry::protocol::{
    INDEX_SCHEMA, RegistryEntry, RegistryIndex, RegistryStore, blake3_hex,
};
use crate::registry::signatures::{SecretKey, sign_pack};

/// One pack listed in a source tree's `manifest.json`.
#[derive(Debug, Deserialize)]
struct ManifestPack {
    name: String,
    category: String,
    version: String,
    summary: String,
}

/// The source-tree manifest (`packs/manifest.json`).
#[derive(Debug, Deserialize)]
struct SourceManifest {
    #[serde(default)]
    packs: Vec<ManifestPack>,
}

/// The default transport: a local directory holding `index.json` +
/// `packs/`. Structurally offline — no network code exists in this crate.
#[derive(Debug, Clone)]
pub struct LocalDirStore {
    root: PathBuf,
}

impl LocalDirStore {
    /// Opens the store rooted at `root`.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl RegistryStore for LocalDirStore {
    fn id(&self) -> &str {
        "local"
    }

    fn index(&self) -> Result<RegistryIndex, RegistryError> {
        let path = self.root.join("index.json");
        let bytes = std::fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => RegistryError::IndexMissing,
            _ => RegistryError::io("read registry index", &e),
        })?;
        RegistryIndex::parse(&bytes)
    }

    fn pack_bytes(&self, hash: &str) -> Result<Vec<u8>, RegistryError> {
        crate::registry::packfile::validate_hash_hex(hash)?;
        let path = self.root.join("packs").join(format!("{hash}.hpk"));
        std::fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => RegistryError::PackFileMissing {
                hash: hash.to_string(),
            },
            _ => RegistryError::io("read pack file", &e),
        })
    }
}

/// Builds the static registry from a packs source tree. Deterministic:
/// identical trees produce byte-identical pack files and hashes, so a
/// registry operator can rebuild and diff. Optional `key` signs every
/// entry (keyed-blake3; see [`crate::registry::signatures`]).
pub fn build_registry_from_packs_dir(
    src: &Path,
    out: &Path,
    key: Option<&SecretKey>,
) -> Result<RegistryIndex, RegistryError> {
    let manifest_bytes = std::fs::read(src.join("manifest.json"))
        .map_err(|e| RegistryError::io("read source manifest", &e))?;
    let manifest: SourceManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| RegistryError::IndexCorrupt {
            detail: format!("source manifest: {e}"),
        })?;
    let shared = collect_shared(&src.join("_shared"))?;
    let packs_dir = out.join("packs");
    std::fs::create_dir_all(&packs_dir).map_err(|e| RegistryError::io("create packs dir", &e))?;

    let mut packs: Vec<ManifestPack> = manifest.packs;
    packs.sort_by(|a, b| a.name.cmp(&b.name));
    let mut entries = Vec::new();
    for pack in &packs {
        validate_item_name(&pack.name)?;
        let packfile = build_pack(src, pack, &shared)?;
        let bytes = packfile.to_bytes();
        let hash = blake3_hex(&bytes);
        std::fs::write(packs_dir.join(format!("{hash}.hpk")), &bytes)
            .map_err(|e| RegistryError::io("write pack file", &e))?;
        entries.push(RegistryEntry {
            name: pack.name.clone(),
            version: pack.version.clone(),
            category: pack.category.clone(),
            summary: pack.summary.clone(),
            hash,
            size: bytes.len() as u64,
            signature: key.map(|k| sign_pack(&bytes, k)),
        });
    }
    let index = RegistryIndex {
        schema: INDEX_SCHEMA,
        entries,
    };
    std::fs::write(out.join("index.json"), index.to_bytes())
        .map_err(|e| RegistryError::io("write index", &e))?;
    Ok(index)
}

/// Publishes one programmatically-built pack (marketplace items: MCP
/// entries + secret params) into an existing registry directory.
pub fn add_pack(
    registry_dir: &Path,
    pack: &PackFile,
    key: Option<&SecretKey>,
) -> Result<RegistryEntry, RegistryError> {
    pack.validate()?;
    let bytes = pack.to_bytes();
    let hash = blake3_hex(&bytes);
    let packs_dir = registry_dir.join("packs");
    std::fs::create_dir_all(&packs_dir).map_err(|e| RegistryError::io("create packs dir", &e))?;
    std::fs::write(packs_dir.join(format!("{hash}.hpk")), &bytes)
        .map_err(|e| RegistryError::io("write pack file", &e))?;
    let entry = RegistryEntry {
        name: pack.name.clone(),
        version: pack.version.clone(),
        hash: hash.clone(),
        size: bytes.len() as u64,
        category: String::new(),
        summary: pack.summary.clone(),
        signature: key.map(|k| sign_pack(&bytes, k)),
    };
    let mut index = match std::fs::read(registry_dir.join("index.json")) {
        Ok(bytes) => RegistryIndex::parse(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => RegistryIndex::default(),
        Err(e) => return Err(RegistryError::io("read index", &e)),
    };
    index.entries.retain(|e| e.name != entry.name);
    index.entries.push(entry.clone());
    std::fs::write(registry_dir.join("index.json"), index.to_bytes())
        .map_err(|e| RegistryError::io("write index", &e))?;
    Ok(entry)
}

fn collect_shared(dir: &Path) -> Result<BTreeMap<String, String>, RegistryError> {
    let mut shared = BTreeMap::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(shared),
        Err(e) => return Err(RegistryError::io("read _shared dir", &e)),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        let text =
            std::fs::read_to_string(&path).map_err(|e| RegistryError::io("read shared", &e))?;
        shared.insert(stem, text);
    }
    Ok(shared)
}

fn build_pack(
    src: &Path,
    pack: &ManifestPack,
    shared: &BTreeMap<String, String>,
) -> Result<PackFile, RegistryError> {
    let mut files = BTreeMap::new();
    let mut rules = BTreeMap::new();
    let mut bundle = None;
    collect_pack_dir(
        &src.join(&pack.name),
        "",
        &mut files,
        &mut rules,
        &mut bundle,
    )?;
    let mut packfile = PackFile::empty(&pack.name, &pack.version);
    packfile.summary = pack.summary.clone();
    packfile.files = files;
    packfile.rules = rules;
    packfile.shared = shared.clone();
    packfile.bundle = bundle;
    packfile.validate()?;
    Ok(packfile)
}

fn collect_pack_dir(
    dir: &Path,
    prefix: &str,
    files: &mut BTreeMap<String, String>,
    rules: &mut BTreeMap<String, String>,
    bundle: &mut Option<BundleManifest>,
) -> Result<(), RegistryError> {
    let entries = std::fs::read_dir(dir).map_err(|e| RegistryError::io("read pack dir", &e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let Some(name) = name else { continue };
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if path.is_dir() {
            collect_pack_dir(&path, &rel, files, rules, bundle)?;
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| RegistryError::io("read pack source", &e))?;
        if rel == "bundle.json" {
            *bundle =
                Some(
                    serde_json::from_slice(&bytes).map_err(|e| RegistryError::PackCorrupt {
                        detail: format!("bundle.json: {e}"),
                    })?,
                );
            continue;
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| RegistryError::NonUtf8PackFile { path: path.clone() })?;
        if let Some(rule_name) = rel.strip_prefix("rules/") {
            rules.insert(rule_name.to_string(), text);
        } else {
            files.insert(rel, text);
        }
    }
    Ok(())
}
