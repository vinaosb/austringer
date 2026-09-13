//! Project rules pickup and precedence: AGENTS.md (standard) + CLAUDE.md
//! (legacy) + `.claude/rules` + `.cursor/rules` + `.harness/rules` (User
//! layer), per-directory stacking (nearest-first, child wins on conflict
//! within a layer, both entries retained), folder-trust gating, and the
//! enforceable TEAM floor (team-marked rules cannot be overridden by
//! project/user rules — lower-layer attempts produce typed conflict
//! reports and lose the merge).
//!
//! Rule grammar (kept deliberately tiny): one rule per bullet line —
//! `- [[rule-id]] rule text` (id optional; unmarked bullets get generated
//! ids and never conflict). A rule whose text starts with the uppercase
//! `TEAM:` marker is a team-floor rule regardless of which file it lives
//! in. All other file content (headings, prose, fences) is ignored.

mod conflict;
mod parse;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::SkillError;

pub use crate::trust::TrustConfig;

pub use conflict::RulesConflict;
pub(crate) use parse::parse_rule_lines;

use conflict::build_conflicts;

/// Precedence layers: Team (floor) > Project > User.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleLayer {
    /// Team floor: cannot be overridden.
    Team,
    /// Project-level rules (AGENTS.md, CLAUDE.md, vendor rules dirs).
    Project,
    /// User-level rules (`.harness/rules`).
    User,
}

impl RuleLayer {
    fn rank(self) -> u8 {
        match self {
            Self::Team => 2,
            Self::Project => 1,
            Self::User => 0,
        }
    }
}

/// One parsed rule.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuleEntry {
    /// Rule id (explicit `[[id]]` or generated).
    pub id: String,
    /// Rule text with the id marker and TEAM prefix stripped.
    pub text: String,
    /// Effective layer (Team when the `TEAM:` marker is present).
    pub layer: RuleLayer,
    /// Source file.
    pub source: PathBuf,
    /// Distance from the workdir (0 = the workdir itself; nearer wins).
    pub depth: u32,
    /// Position within its file (tie-breaker).
    pub order: u32,
}

/// A typed notice about a skipped rules file.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleNotice {
    /// The skipped file/directory.
    pub path: PathBuf,
    /// Why it was skipped.
    pub kind: NoticeKind,
}

impl RuleNotice {
    /// True when the notice is an untrusted-directory skip.
    pub fn is_untrusted_dir(&self) -> bool {
        matches!(self.kind, NoticeKind::UntrustedDir)
    }
}

/// Notice taxonomy.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum NoticeKind {
    /// The directory is not in the typed trust set.
    #[error("untrusted directory: rules skipped")]
    UntrustedDir,
    /// The file exists but could not be read.
    #[error("unreadable: {0}")]
    Unreadable(String),
}

/// The merged rules stack with conflicts and notices.
#[derive(Debug, Clone, Default)]
pub struct RulesStack {
    /// Every retained rule (winners and losers).
    pub entries: Vec<RuleEntry>,
    /// Skipped-file notices.
    pub notices: Vec<RuleNotice>,
    /// Conflict report (typed).
    pub conflicts: Vec<RulesConflict>,
}

impl RulesStack {
    /// The winning entry for an id (Team floor first, then nearest).
    pub fn winner(&self, id: &str) -> Option<&RuleEntry> {
        self.entries
            .iter()
            .filter(|e| e.id == id)
            .min_by(|a, b| self.rank(a).cmp(&self.rank(b)))
    }

    /// Winning entries for all ids, sorted by id.
    pub fn winners(&self) -> impl Iterator<Item = &RuleEntry> {
        let mut ids: Vec<&str> = self.entries.iter().map(|e| e.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        ids.into_iter().filter_map(move |id| self.winner(id))
    }

    fn rank(&self, e: &RuleEntry) -> (u8, u32, u32) {
        (u8::MAX - e.layer.rank(), e.depth, e.order)
    }
}

/// Loads the rules stack for work under `workdir`: walks toward the
/// filesystem root (nearest-first), picking up every trusted directory's
/// rule files.
pub fn load_rules(workdir: &Path, trust: &TrustConfig) -> Result<RulesStack, SkillError> {
    let start = workdir
        .canonicalize()
        .map_err(|e| SkillError::io("canonicalize workdir", &e))?;
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut cur = Some(start);
    while let Some(d) = cur {
        dirs.push(d.clone());
        cur = d.parent().map(Path::to_path_buf);
    }

    let mut stack = RulesStack::default();
    for (depth, dir) in dirs.iter().enumerate() {
        if !trust.is_trusted(dir) {
            stack.notices.push(RuleNotice {
                path: dir.clone(),
                kind: NoticeKind::UntrustedDir,
            });
            continue;
        }
        let depth = depth as u32;
        // Same-dir precedence: AGENTS.md before CLAUDE.md; rules dirs last.
        collect_file(
            &mut stack,
            &dir.join("AGENTS.md"),
            RuleLayer::Project,
            depth,
        )?;
        collect_file(
            &mut stack,
            &dir.join("CLAUDE.md"),
            RuleLayer::Project,
            depth,
        )?;
        collect_rules_dir(
            &mut stack,
            &dir.join(".claude").join("rules"),
            RuleLayer::Project,
            depth,
        )?;
        collect_rules_dir(
            &mut stack,
            &dir.join(".cursor").join("rules"),
            RuleLayer::Project,
            depth,
        )?;
        collect_rules_dir(
            &mut stack,
            &dir.join(".harness").join("rules"),
            RuleLayer::User,
            depth,
        )?;
    }
    stack.conflicts = build_conflicts(&stack);
    Ok(stack)
}

fn collect_rules_dir(
    stack: &mut RulesStack,
    dir: &Path,
    layer: RuleLayer,
    depth: u32,
) -> Result<(), SkillError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(SkillError::io("read rules dir", &e)),
    };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    files.sort();
    for file in files {
        collect_file(stack, &file, layer, depth)?;
    }
    Ok(())
}

fn collect_file(
    stack: &mut RulesStack,
    path: &Path,
    layer: RuleLayer,
    depth: u32,
) -> Result<(), SkillError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            stack.notices.push(RuleNotice {
                path: path.to_path_buf(),
                kind: NoticeKind::Unreadable(e.to_string()),
            });
            return Ok(());
        }
    };
    let parsed = parse_rule_lines(path, &text, layer, depth);
    let base = stack.entries.len() as u32;
    for (offset, entry) in parsed.into_iter().enumerate() {
        stack.entries.push(RuleEntry {
            order: base + offset as u32,
            ..entry
        });
    }
    Ok(())
}
