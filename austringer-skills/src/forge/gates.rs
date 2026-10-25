//! Draft tier + approval gate + eval-before-activate (write_approval).
//! Drafts persist in a namespace the T30 loader never scans; the only
//! path from draft to routable skill is
//! [`ActivationGate::decide`] with an explicit
//! [`ApprovalDecision::Approve`] AND a passing replay eval. Rejection
//! purges; eval failure blocks with a typed report; existing skills
//! change only through deltas with version checks (no silent overwrite
//! is representable).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::forge::ForgeError;
use crate::forge::delta::apply_delta;
use crate::forge::distill::DraftRecord;
use crate::forge::document::{extract_body, parse_body_sections, render_document};
use crate::forge::eval::{EvalReport, replay_draft};
use crate::skill::{SkillName, parse_skill_document};

/// The human decision on a draft (Hermes write-approval gate).
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalDecision {
    /// Approve activation (still requires a passing eval).
    Approve {
        /// Who approved.
        approver: String,
    },
    /// Reject: the draft is purged and leaves nothing behind.
    Reject {
        /// Who rejected.
        approver: String,
        /// Why.
        reason: String,
    },
}

/// The disk-backed draft namespace (`<root>/<id>.draft.json`).
#[derive(Debug, Clone)]
pub struct DraftStore {
    root: PathBuf,
}

impl DraftStore {
    /// A draft store rooted at `root` (created on first stage).
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Path of one draft file (whether or not it exists).
    pub fn draft_file(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }

    /// Persists a draft (inert: this directory is never loader-scanned).
    pub fn stage(&self, record: &DraftRecord) -> Result<(), ForgeError> {
        std::fs::create_dir_all(&self.root).map_err(|e| ForgeError::io("create drafts dir", &e))?;
        let bytes = serde_json::to_vec_pretty(record).map_err(|e| ForgeError::DraftCorrupt {
            detail: e.to_string(),
        })?;
        std::fs::write(self.draft_file(&record.id), bytes)
            .map_err(|e| ForgeError::io("write draft", &e))
    }

    /// Loads one draft.
    pub fn get(&self, id: &str) -> Result<DraftRecord, ForgeError> {
        let bytes = std::fs::read(self.draft_file(id)).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ForgeError::DraftNotFound { id: id.to_string() },
            _ => ForgeError::io("read draft", &e),
        })?;
        serde_json::from_slice(&bytes).map_err(|e| ForgeError::DraftCorrupt {
            detail: e.to_string(),
        })
    }

    /// All staged drafts (sorted by file name ⇒ stable order).
    pub fn list(&self) -> Result<Vec<DraftRecord>, ForgeError> {
        let mut ids: Vec<PathBuf> = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(ForgeError::io("list drafts", &e)),
        };
        ids.sort();
        ids.iter()
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .map(|id| self.get(&id))
            .collect()
    }

    /// Removes a draft file; purging an unknown draft is a no-op.
    pub fn purge(&self, id: &str) -> Result<(), ForgeError> {
        match std::fs::remove_file(self.draft_file(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ForgeError::io("purge draft", &e)),
        }
    }
}

/// A successfully activated skill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activation {
    /// Activated skill name.
    pub skill: SkillName,
    /// Version written.
    pub version: String,
    /// The SKILL.md path (under the skills root).
    pub skill_path: PathBuf,
}

/// The outcome of a decision on a draft.
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionOutcome {
    /// Approved AND eval-passed: SKILL.md written, draft purged.
    Activated(Activation),
    /// Rejected: draft purged, nothing written anywhere else.
    RejectedPurged {
        /// The purged draft id.
        id: String,
    },
    /// Approved but the replay eval failed: nothing written; the draft
    /// stays staged for revision.
    Blocked {
        /// The typed eval report.
        report: EvalReport,
    },
}

/// The activation gate binding a draft namespace to a skills root.
#[derive(Debug, Clone)]
pub struct ActivationGate {
    drafts: DraftStore,
    skills_root: PathBuf,
}

impl ActivationGate {
    /// Gate over one draft store and one skills directory.
    pub fn new(drafts: DraftStore, skills_root: PathBuf) -> Self {
        Self {
            drafts,
            skills_root,
        }
    }

    /// Applies a human decision to a staged draft.
    pub fn decide(
        &self,
        id: &str,
        decision: ApprovalDecision,
    ) -> Result<DecisionOutcome, ForgeError> {
        if let ApprovalDecision::Reject { .. } = decision {
            self.drafts.get(id)?;
            self.drafts.purge(id)?;
            return Ok(DecisionOutcome::RejectedPurged { id: id.to_string() });
        }
        let draft = self.drafts.get(id)?;
        if draft.is_inert() {
            return Err(ForgeError::DraftInert {
                id: id.to_string(),
                findings: draft.findings.len(),
            });
        }
        let skill_dir = self.skills_root.join(draft.proposal.spec.name.as_str());
        let live_path = skill_dir.join("SKILL.md");
        let document = self.render_target(&draft, &live_path)?;
        let report = replay_draft(&document, &draft.replay_cases);
        if !report.is_pass() {
            return Ok(DecisionOutcome::Blocked { report });
        }
        std::fs::create_dir_all(&skill_dir).map_err(|e| ForgeError::io("create skill dir", &e))?;
        std::fs::write(&live_path, &document).map_err(|e| ForgeError::io("write SKILL.md", &e))?;
        let version = draft.delta.as_ref().map_or_else(
            || draft.proposal.spec.version.clone(),
            |d| d.to_version.clone(),
        );
        self.drafts.purge(id)?;
        Ok(DecisionOutcome::Activated(Activation {
            skill: draft.proposal.spec.name.clone(),
            version,
            skill_path: live_path,
        }))
    }

    /// Builds the document the eval replays against — new-skill render
    /// or delta merge, never a whole-skill rewrite of an existing file.
    fn render_target(&self, draft: &DraftRecord, live_path: &Path) -> Result<String, ForgeError> {
        let live = match std::fs::read_to_string(live_path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(ForgeError::io("read live SKILL.md", &e)),
        };
        match (live, &draft.delta, &draft.from_version) {
            (None, _, None) => Ok(render_document(
                &draft.proposal.spec,
                &draft.proposal.sections,
            )),
            (None, _, Some(from)) => Err(ForgeError::StaleDraft {
                skill: draft.proposal.spec.name.to_string(),
                expected: from.clone(),
                actual: "<skill deleted>".to_string(),
            }),
            (Some(_), None, _) => Err(ForgeError::OverwriteRequiresDelta {
                skill: draft.proposal.spec.name.to_string(),
            }),
            (Some(current), Some(delta), Some(from)) => {
                let (meta, _) = parse_skill_document(live_path, "skill", current.as_bytes())
                    .map_err(|e| ForgeError::DraftCorrupt {
                        detail: format!("live SKILL.md no longer parses: {e}"),
                    })?;
                let actual = meta.version.clone().unwrap_or_default();
                if actual != *from || delta.from_version != *from {
                    return Err(ForgeError::StaleDraft {
                        skill: draft.proposal.spec.name.to_string(),
                        expected: from.clone(),
                        actual,
                    });
                }
                let merged = apply_delta(&parse_body_sections(extract_body(&current)), delta)?;
                let mut spec = draft.proposal.spec.clone();
                spec.version = delta.to_version.clone();
                Ok(render_document(&spec, &merged))
            }
            (Some(_), Some(_), None) => Err(ForgeError::OverwriteRequiresDelta {
                skill: draft.proposal.spec.name.to_string(),
            }),
        }
    }
}
