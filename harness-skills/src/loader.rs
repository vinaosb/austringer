//! Multi-vendor skill loader: scans `.harness`/`.opencode`/`.claude`/
//! `.agents`/`.cursor` skills paths (plus explicit skills dirs), dedups
//! cross-vendor by name with precedence, auto-scopes nested monorepo
//! skills to their subtree, collects `_shared` pack partials, applies
//! skills-lock pinning, and exposes metadata-only routing with bodies
//! available solely through explicit invoke (progressive disclosure).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::SkillError;
use crate::lock::LockMismatch;
use crate::skill::{SkillParseError, SkillWarning};

use crate::index::IndexEntry;
pub use crate::index::{InvokedSkill, SkillIndex};

/// Vendor dirs in precedence order (highest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Vendor {
    /// `.cursor` (lowest precedence).
    Cursor,
    /// `.agents`.
    Agents,
    /// `.claude`.
    Claude,
    /// `.opencode`.
    Opencode,
    /// `.harness` (highest precedence).
    Harness,
}

impl Vendor {
    /// The vendor's dot-dir name.
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Cursor => ".cursor",
            Self::Agents => ".agents",
            Self::Claude => ".claude",
            Self::Opencode => ".opencode",
            Self::Harness => ".harness",
        }
    }

    pub(crate) fn from_dir_name(name: &str) -> Option<Self> {
        match name {
            ".cursor" => Some(Self::Cursor),
            ".agents" => Some(Self::Agents),
            ".claude" => Some(Self::Claude),
            ".opencode" => Some(Self::Opencode),
            ".harness" => Some(Self::Harness),
            _ => None,
        }
    }
}

const VENDORS_HIGH_TO_LOW: [Vendor; 5] = [
    Vendor::Harness,
    Vendor::Opencode,
    Vendor::Claude,
    Vendor::Agents,
    Vendor::Cursor,
];

/// Lock behavior during load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LockMode {
    /// No lock file is read or written (read-only fixture loads).
    #[default]
    Off,
    /// Pins are recorded on first sight and verified afterwards;
    /// mismatches shadow the skill out of routing.
    Record,
}

/// A lower-priority occurrence of a deduped skill name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShadowRecord {
    /// The deduped name.
    pub name: String,
    /// Vendor of the shadowed occurrence.
    pub vendor: Vendor,
    /// Path of the shadowed SKILL.md.
    pub path: PathBuf,
}

/// Scan outcome summary.
#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    /// Skills successfully parsed.
    pub parsed: usize,
    /// Typed parse errors (scan never aborts on them).
    pub parse_errors: Vec<SkillParseError>,
    /// Typed, non-fatal warnings.
    pub warnings: Vec<SkillWarning>,
    /// Deduped lower-priority occurrences, ascending by precedence.
    pub shadows: Vec<ShadowRecord>,
    /// Lock pin mismatches.
    pub lock_mismatches: Vec<LockMismatch>,
}

/// Loader builder. `vendor_root(base)` scans base/{.harness,.opencode,
/// .claude,.agents,.cursor}/skills (plus nested monorepo skill dirs deeper
/// under base); `skills_dir(dir, vendor)` mounts a directory that already
/// IS a skills dir.
#[derive(Debug, Clone, Default)]
pub struct SkillLoader {
    roots: Vec<(PathBuf, Option<Vendor>)>,
    lock: LockMode,
}

impl SkillLoader {
    /// An empty loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a base directory scanned for the five vendor skills paths.
    pub fn vendor_root(&mut self, base: impl AsRef<Path>) -> &mut Self {
        self.roots.push((base.as_ref().to_path_buf(), None));
        self
    }

    /// Adds a directory that is itself a skills dir, tagged with a vendor.
    pub fn skills_dir(&mut self, dir: impl AsRef<Path>, vendor: Vendor) -> &mut Self {
        self.roots.push((dir.as_ref().to_path_buf(), Some(vendor)));
        self
    }

    /// Selects lock behavior.
    pub fn lock_mode(&mut self, mode: LockMode) -> &mut Self {
        self.lock = mode;
        self
    }

    /// Scans all roots and builds the index.
    pub fn load(&self) -> Result<SkillIndex, SkillError> {
        let mut state = crate::scan::ScanState::default();
        let mut report = ScanReport::default();
        let mut lock_root: Option<PathBuf> = None;

        for (root, explicit_vendor) in &self.roots {
            match explicit_vendor {
                Some(vendor) => {
                    if lock_root.is_none() {
                        lock_root = root.parent().map(|p| p.to_path_buf());
                    }
                    state.scan_skills_dir(root, *vendor, None)?;
                }
                None => {
                    if lock_root.is_none() {
                        lock_root = Some(root.clone());
                    }
                    for vendor in VENDORS_HIGH_TO_LOW {
                        let dir = root.join(vendor.dir_name()).join("skills");
                        state.scan_skills_dir(&dir, vendor, None)?;
                    }
                    state.scan_nested_vendor_dirs(root)?;
                }
            }
        }

        for (_, warnings) in &state.found {
            report.warnings.extend(warnings.iter().cloned());
        }
        report.parse_errors = std::mem::take(&mut state.parse_errors);
        let winners = crate::scan::dedup(std::mem::take(&mut state.found), &mut report);
        report.parsed = winners.len();

        let mut index = SkillIndex {
            entries: winners
                .into_iter()
                .map(|(name, (meta, skill_md))| (name, IndexEntry { meta, skill_md }))
                .collect(),
            partials: state.partials,
            partial_paths: state.partial_paths,
            shared_dir: state.shared_dir,
            report,
            routable: Vec::new(),
        };
        if self.lock == LockMode::Record {
            if let Some(root) = lock_root {
                index.apply_lock(&root.join(".harness").join("skills-lock.json"))?;
            }
        } else {
            index.routable = index.entries.values().map(|e| e.meta.clone()).collect();
        }
        Ok(index)
    }
}
