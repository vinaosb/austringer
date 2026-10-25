//! skills-lock.json pinning: content-hash pins written on first load,
//! verified on every load. A pin mismatch downgrades the skill to a
//! non-routable shadow with a typed warning — the pin is never silently
//! rewritten (re-recording is an explicit human action).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::SkillError;
use crate::skill::SkillName;

/// One pin: skill name → content hash of its SKILL.md.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinRecord {
    /// blake3 hex hash of the pinned SKILL.md bytes.
    pub content_hash: String,
    /// Vendor dir the pin was recorded from.
    pub source: String,
}

/// The lock file shape (sorted-key stable: BTreeMap).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SkillsLockFile {
    /// Lock format version.
    pub version: u32,
    /// name → pin.
    pub pins: BTreeMap<String, PinRecord>,
}

/// A pin that no longer matches the on-disk content.
#[derive(Debug, Clone, PartialEq)]
pub struct LockMismatch {
    /// The mismatched skill.
    pub name: SkillName,
    /// Hash recorded in the lock file.
    pub pinned_hash: String,
    /// Hash of the current file.
    pub actual_hash: String,
}

impl SkillsLockFile {
    /// Reads a lock file; `Ok(None)` when it does not exist.
    pub fn read(path: &Path) -> Result<Option<Self>, SkillError> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(SkillError::io("read skills-lock.json", &e)),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| SkillError::LockCorrupt {
                detail: e.to_string(),
            })
    }

    /// Writes the lock file atomically (tmp + rename).
    pub fn write(&self, path: &Path) -> Result<(), SkillError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SkillError::io("create lock dir", &e))?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| SkillError::LockCorrupt {
            detail: e.to_string(),
        })?;
        std::fs::write(&tmp, bytes).map_err(|e| SkillError::io("write lock tmp", &e))?;
        std::fs::rename(&tmp, path).map_err(|e| SkillError::io("commit lock file", &e))
    }

    /// blake3 hex of SKILL.md bytes.
    pub fn hash_bytes(bytes: &[u8]) -> String {
        blake3::hash(bytes).to_hex().to_string()
    }
}
