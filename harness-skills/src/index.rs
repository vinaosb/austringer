//! The loaded skill index: metadata surface, explicit invoke, routing
//! delegates, and skills-lock application. Holds NO bodies — they are
//! re-read from disk only on [`SkillIndex::invoke`] (progressive
//! disclosure is structural: no body text can reach a caller that did not
//! explicitly invoke a skill).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::consent::ConsentGate;
use crate::error::SkillError;
use crate::loader::ScanReport;
use crate::lock::{LockMismatch, PinRecord, SkillsLockFile};
use crate::routing::{RouteQuery, RoutingDecision, route, route_with_consent};
use crate::skill::{SkillMeta, SkillName};

/// A fully-invoked skill: body plus the invoke-gated surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct InvokedSkill {
    /// Skill name.
    pub name: SkillName,
    /// Full body (frontmatter stripped, `{{shared:...}}` resolved).
    pub body: String,
    /// Declared allowed tools.
    pub allowed_tools: Vec<String>,
    /// Per-model guidance sections.
    pub model_guidance: Vec<crate::skill::ModelGuidance>,
    /// Partials interpolated into the body.
    pub resolved_partials: Vec<crate::pack::ResolvedPartial>,
}

#[derive(Debug, Clone)]
pub(crate) struct IndexEntry {
    pub(crate) meta: SkillMeta,
    pub(crate) skill_md: PathBuf,
}

/// The loaded skill index. Construct via
/// [`crate::loader::SkillLoader::load`].
#[derive(Debug, Clone)]
pub struct SkillIndex {
    pub(crate) entries: BTreeMap<SkillName, IndexEntry>,
    pub(crate) partials: BTreeMap<String, String>,
    pub(crate) partial_paths: Vec<PathBuf>,
    pub(crate) shared_dir: Option<PathBuf>,
    /// Report of the load that produced this index.
    pub(crate) report: ScanReport,
    pub(crate) routable: Vec<SkillMeta>,
}

impl SkillIndex {
    /// An index with no entries, partials, or routing surface — the
    /// fail-soft value for embedders whose scan errored.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: BTreeMap::new(),
            partials: BTreeMap::new(),
            partial_paths: Vec::new(),
            shared_dir: None,
            report: ScanReport::default(),
            routable: Vec::new(),
        }
    }

    /// The scan report.
    pub fn report(&self) -> &ScanReport {
        &self.report
    }

    /// A skill's metadata surface.
    pub fn meta(&self, name: &str) -> Option<&SkillMeta> {
        let key = SkillName::new(name)?;
        self.entries.get(&key).map(|e| &e.meta)
    }

    /// All metadata surfaces.
    pub fn metas(&self) -> impl Iterator<Item = &SkillMeta> {
        self.entries.values().map(|e| &e.meta)
    }

    /// The metadata-only JSON surface (never contains bodies).
    pub fn metas_json(&self) -> serde_json::Value {
        serde_json::to_value(self.metas().collect::<Vec<_>>()).unwrap_or(serde_json::Value::Null)
    }

    /// Explicit invoke: loads the body, allowed tools, and per-model
    /// guidance for one skill (progressive disclosure).
    pub fn invoke(&self, name: &str) -> Result<InvokedSkill, SkillError> {
        let key = SkillName::new(name).ok_or_else(|| SkillError::Io {
            detail: format!("invalid skill name {name:?}"),
        })?;
        let entry = self.entries.get(&key).ok_or_else(|| SkillError::Io {
            detail: format!("skill {name:?} not found"),
        })?;
        let bytes =
            std::fs::read(&entry.skill_md).map_err(|e| SkillError::io("read SKILL.md", &e))?;
        let text = String::from_utf8_lossy(&bytes);
        let body = body_after_frontmatter(&text);
        let shared_dir = self
            .shared_dir
            .clone()
            .unwrap_or_else(|| entry.meta.skill_dir.join("_shared"));
        let (resolved, partials_used) =
            crate::pack::resolve_shared(&body, &self.partials, &shared_dir)?;
        Ok(InvokedSkill {
            name: entry.meta.name.clone(),
            body: resolved,
            allowed_tools: entry.meta.allowed_tools.clone(),
            model_guidance: entry.meta.model_guidance.clone(),
            resolved_partials: partials_used,
        })
    }

    /// Routes a prompt (no consent gate).
    pub fn route_text(&self, text: &str, work_path: Option<&Path>) -> RoutingDecision {
        let query = RouteQuery { text, work_path };
        route(&self.routable, &query)
    }

    /// Routes a prompt behind a consent gate.
    pub fn route_with(
        &self,
        query: &RouteQuery<'_>,
        gate: Option<&ConsentGate>,
    ) -> RoutingDecision {
        match gate {
            Some(g) => route_with_consent(&self.routable, query, g),
            None => route(&self.routable, query),
        }
    }

    /// Paths of discovered `_shared` partial files.
    pub fn shared_partials(&self) -> impl Iterator<Item = &PathBuf> {
        self.partial_paths.iter()
    }

    /// Applies lock pinning: records pins for unseen skills, flags
    /// mismatches (never rewrites a mismatched pin), and shadows
    /// mismatched skills out of routing.
    pub(crate) fn apply_lock(&mut self, lock_path: &Path) -> Result<(), SkillError> {
        let mut lock = SkillsLockFile::read(lock_path)?.unwrap_or_default();
        lock.version = 1;
        let mut changed = false;
        for entry in self.entries.values() {
            let bytes = std::fs::read(&entry.skill_md)
                .map_err(|e| SkillError::io("read SKILL.md for pin", &e))?;
            let actual = SkillsLockFile::hash_bytes(&bytes);
            match lock.pins.get(entry.meta.name.as_str()) {
                Some(pin) if pin.content_hash != actual => {
                    self.report.lock_mismatches.push(LockMismatch {
                        name: entry.meta.name.clone(),
                        pinned_hash: pin.content_hash.clone(),
                        actual_hash: actual,
                    });
                }
                Some(_) => {}
                None => {
                    lock.pins.insert(
                        entry.meta.name.to_string(),
                        PinRecord {
                            content_hash: actual,
                            source: entry.meta.vendor.dir_name().to_string(),
                        },
                    );
                    changed = true;
                }
            }
        }
        if SkillsLockFile::read(lock_path)?.is_none() || changed {
            lock.write(lock_path)?;
        }
        let mismatched: Vec<String> = self
            .report
            .lock_mismatches
            .iter()
            .map(|m| m.name.to_string())
            .collect();
        self.routable = self
            .entries
            .values()
            .filter(|e| !mismatched.contains(&e.meta.name.to_string()))
            .map(|e| e.meta.clone())
            .collect();
        Ok(())
    }
}

fn body_after_frontmatter(text: &str) -> String {
    let trimmed = text.trim_start_matches('\u{FEFF}');
    let lines: Vec<&str> = trimmed
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    if lines.first().is_none_or(|l| l.trim_end() != "---") {
        return trimmed.to_string();
    }
    for (idx, line) in lines.iter().enumerate().skip(1) {
        if line.trim_end() == "---" {
            return lines[idx + 1..]
                .join("\n")
                .trim_start_matches('\n')
                .to_string();
        }
    }
    trimmed.to_string()
}
