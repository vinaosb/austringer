//! Filesystem discovery for the loader: vendor skills dirs, recursive
//! SKILL.md descent (nested skill trees like `microsoft-foundry`),
//! `_shared` partial pickup, nested monorepo vendor dirs with subtree
//! scopes, and cross-vendor dedup-by-name.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::SkillError;
use crate::loader::{ScanReport, ShadowRecord, Vendor};
use crate::lock::SkillsLockFile;
use crate::skill::{SkillMeta, SkillName, SkillParseError, SkillWarning, parse_skill_document};

const MAX_DEPTH: u32 = 8;
const SKIP_DIRS: [&str; 3] = ["node_modules", "target", "__pycache__"];

/// Accumulates discovery output for one load run.
#[derive(Debug, Default)]
pub(crate) struct ScanState {
    /// (meta, blake3 content hash of the SKILL.md bytes, warnings).
    pub found: Vec<(SkillMeta, String, Vec<SkillWarning>)>,
    pub parse_errors: Vec<SkillParseError>,
    pub partials: BTreeMap<String, String>,
    pub partial_paths: Vec<PathBuf>,
    pub shared_dir: Option<PathBuf>,
}

impl ScanState {
    /// Scans one skills dir (already resolved to `{vendor}/skills` or an
    /// explicit dir). `scope` is `None` for top-level dirs and the owning
    /// subtree for nested monorepo skills.
    pub fn scan_skills_dir(
        &mut self,
        dir: &Path,
        vendor: Vendor,
        scope: Option<&Path>,
    ) -> Result<(), SkillError> {
        self.walk(dir, vendor, scope, 0)
    }

    fn walk(
        &mut self,
        dir: &Path,
        vendor: Vendor,
        scope: Option<&Path>,
        depth: u32,
    ) -> Result<(), SkillError> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(SkillError::io("scan dir", &e)),
        };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let Ok(ftype) = entry.file_type() else {
                continue;
            };
            if ftype.is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                if name.starts_with('_') && name != "_shared" {
                    continue;
                }
                subdirs.push(path);
            } else if name == "SKILL.md" {
                let dir_name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "skill".to_string());
                match std::fs::read(&path) {
                    Ok(bytes) => match parse_skill_document(&path, &dir_name, &bytes) {
                        Ok((mut meta, warnings)) => {
                            let content_hash = SkillsLockFile::hash_bytes(&bytes);
                            meta.vendor = vendor;
                            meta.scope = scope.map(Path::to_path_buf);
                            meta.skill_dir = dir.to_path_buf();
                            self.found.push((meta, content_hash, warnings));
                        }
                        Err(kind) => self.parse_errors.push(kind),
                    },
                    Err(e) => self.parse_errors.push(SkillParseError {
                        path,
                        line: 0,
                        kind: crate::skill::ParseErrorKind::ReadFailed(e.to_string()),
                    }),
                }
            } else if dir.ends_with("_shared") && name.ends_with(".md") {
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
                if let (Some(stem), Ok(content)) = (stem, std::fs::read_to_string(&path)) {
                    self.shared_dir.get_or_insert_with(|| dir.to_path_buf());
                    self.partials.insert(stem.clone(), content);
                    self.partial_paths.push(path);
                }
            }
        }
        subdirs.sort();
        for sub in subdirs {
            self.walk(&sub, vendor, scope, depth + 1)?;
        }
        Ok(())
    }

    /// Walks `base` looking for vendor dirs BELOW the top level; each
    /// `{subtree}/{vendor}/skills` becomes a scoped skill set.
    pub fn scan_nested_vendor_dirs(&mut self, base: &Path) -> Result<(), SkillError> {
        let mut stack = vec![(base.to_path_buf(), 0u32)];
        while let Some((dir, depth)) = stack.pop() {
            if depth > MAX_DEPTH {
                continue;
            }
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(SkillError::io("nested scan", &e)),
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let Ok(ftype) = entry.file_type() else {
                    continue;
                };
                if !ftype.is_dir() {
                    continue;
                }
                if let Some(vendor) = Vendor::from_dir_name(&name) {
                    if depth == 0 {
                        continue;
                    }
                    let scope = dir.clone();
                    self.scan_skills_dir(&path.join("skills"), vendor, Some(&scope))?;
                    continue;
                }
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                stack.push((path, depth + 1));
            }
        }
        Ok(())
    }
}

/// Dedups discovered skills by name: highest vendor precedence wins, ties
/// keep the first scanned; losers become shadow records (ascending
/// precedence order). The winner's scan-time content hash rides through
/// to the index entry. Warnings were already drained by the caller.
pub(crate) fn dedup(
    found: Vec<(SkillMeta, String, Vec<SkillWarning>)>,
    report: &mut ScanReport,
) -> BTreeMap<SkillName, (SkillMeta, String, PathBuf)> {
    let mut winners: BTreeMap<SkillName, (SkillMeta, String, PathBuf)> = BTreeMap::new();
    let mut shadows: Vec<ShadowRecord> = Vec::new();
    for (meta, content_hash, _warnings) in found {
        let skill_md = meta.skill_dir.join("SKILL.md");
        let lost_to = winners.get(&meta.name).map(|(w, _, _)| w.vendor);
        let replace = match lost_to {
            Some(existing) => meta.vendor > existing,
            None => true,
        };
        if replace {
            if let Some((old, _, _)) =
                winners.insert(meta.name.clone(), (meta, content_hash, skill_md))
            {
                shadows.push(ShadowRecord {
                    name: old.name.to_string(),
                    vendor: old.vendor,
                    path: old.skill_dir.join("SKILL.md"),
                });
            }
        } else {
            shadows.push(ShadowRecord {
                name: meta.name.to_string(),
                vendor: meta.vendor,
                path: skill_md,
            });
        }
    }
    shadows.sort_by_key(|s| (s.vendor, s.name.clone()));
    report.shadows = shadows;
    winners
}
