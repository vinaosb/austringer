//! One-time CLAUDE.md→AGENTS.md migration (copies, never moves, with a
//! marker; idempotent), first-init of the per-harness global dirs, and
//! `/migrate-to-skills-style` converters for legacy rules files and
//! slash-command dirs (typed conversion reports; lossy conversions flagged).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::SkillError;
use crate::rules::{RuleEntry, parse_rule_lines};
use crate::skill::{DESCRIPTION_MAX, SkillName};

/// Marker written into migrated AGENTS.md files (also the idempotence check).
pub const MIGRATION_MARKER: &str =
    "<!-- migrated-from: CLAUDE.md (harness-skills one-time copy; source file untouched) -->";

/// Why a migration did not create anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SkipReason {
    /// AGENTS.md already exists (standard file wins; migration is a no-op).
    AgentsAlreadyExists,
    /// No CLAUDE.md in the directory.
    NoClaudeMd,
}

/// Migration outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationReport {
    /// The AGENTS.md that was created, if any.
    pub created: Option<std::path::PathBuf>,
    /// Why nothing was created.
    pub skipped_reason: Option<SkipReason>,
}

/// Copies CLAUDE.md content into AGENTS.md shape with the migration
/// marker. Never moves or deletes the source. Idempotent.
pub fn migrate_claude_to_agents(dir: &Path) -> Result<MigrationReport, SkillError> {
    let agents = dir.join("AGENTS.md");
    let claude = dir.join("CLAUDE.md");
    if agents.exists() {
        return Ok(MigrationReport {
            created: None,
            skipped_reason: Some(SkipReason::AgentsAlreadyExists),
        });
    }
    let content = match std::fs::read_to_string(&claude) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MigrationReport {
                created: None,
                skipped_reason: Some(SkipReason::NoClaudeMd),
            });
        }
        Err(e) => return Err(SkillError::io("read CLAUDE.md", &e)),
    };
    let merged = format!("{MIGRATION_MARKER}\n\n{content}");
    std::fs::write(&agents, merged).map_err(|e| SkillError::io("write AGENTS.md", &e))?;
    Ok(MigrationReport {
        created: Some(agents),
        skipped_reason: None,
    })
}

/// First init of the per-harness global dirs (`.harness/skills`,
/// `.harness/rules`) plus the one-time CLAUDE.md migration (Warp pattern).
pub fn first_init(base: &Path) -> Result<MigrationReport, SkillError> {
    for dir in [
        base.join(".harness").join("skills"),
        base.join(".harness").join("rules"),
    ] {
        std::fs::create_dir_all(&dir).map_err(|e| SkillError::io("create harness dir", &e))?;
    }
    migrate_claude_to_agents(base)
}

/// A lossy conversion (content did not map 1:1 onto SKILL.md shape).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LossyConversion {
    /// The source that was converted lossily.
    pub source: std::path::PathBuf,
    /// Why it was lossy.
    pub reason: String,
}

/// Converter output.
#[derive(Debug, Clone, Default)]
pub struct ConversionReport {
    /// Skill dirs created, sorted by skill name.
    pub created: Vec<std::path::PathBuf>,
    /// Lossy conversions (flagged, never silent).
    pub lossy: Vec<LossyConversion>,
    /// Inputs skipped entirely (e.g. empty command files).
    pub skipped: Vec<std::path::PathBuf>,
}

/// Converts a legacy rules file (bullet grammar from [`crate::rules`])
/// into one SKILL.md per rule id under `out_dir`.
pub fn convert_rules_to_skills(
    rules_path: &Path,
    out_dir: &Path,
) -> Result<ConversionReport, SkillError> {
    let text =
        std::fs::read_to_string(rules_path).map_err(|e| SkillError::io("read legacy rules", &e))?;
    let entries = parse_rule_lines(rules_path, &text, crate::rules::RuleLayer::Project, 0);
    let mut report = ConversionReport::default();
    let mut by_name: Vec<(SkillName, &RuleEntry)> = Vec::new();
    for entry in &entries {
        let sanitized = SkillName::sanitize(&format!("rule-{}", entry.id));
        if sanitized.as_str() != format!("rule-{}", entry.id) {
            report.lossy.push(LossyConversion {
                source: rules_path.to_path_buf(),
                reason: format!(
                    "rule id {:?} sanitized to {:?}",
                    entry.id,
                    sanitized.as_str()
                ),
            });
        }
        by_name.push((sanitized, entry));
    }
    by_name.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    for (name, entry) in by_name {
        let dir = out_dir.join(name.as_str());
        std::fs::create_dir_all(&dir).map_err(|e| SkillError::io("create converted dir", &e))?;
        let description = if entry.text.len() > DESCRIPTION_MAX {
            report.lossy.push(LossyConversion {
                source: rules_path.to_path_buf(),
                reason: "description truncated to 1024 chars".to_string(),
            });
            entry.text.chars().take(DESCRIPTION_MAX).collect::<String>()
        } else {
            entry.text.clone()
        };
        let doc = format!(
            "---\nname: {}\ndescription: {}\ntriggers: [\"{}\"]\nversion: \"0.1.0\"\n---\n\n# Converted rule {}\n\n- [[{}]] {}\n",
            name.as_str(),
            escape_yaml(&description),
            entry.id,
            entry.id,
            entry.id,
            entry.text
        );
        std::fs::write(dir.join("SKILL.md"), doc)
            .map_err(|e| SkillError::io("write converted SKILL.md", &e))?;
        report.created.push(dir);
    }
    Ok(report)
}

/// Converts a `.claude/commands`-style slash-command dir into skills: one
/// skill per markdown file, trigger `/name`, body preserved verbatim.
pub fn convert_commands_to_skills(
    commands_dir: &Path,
    out_dir: &Path,
) -> Result<ConversionReport, SkillError> {
    let entries = match std::fs::read_dir(commands_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConversionReport::default());
        }
        Err(e) => return Err(SkillError::io("read commands dir", &e)),
    };
    let mut report = ConversionReport::default();
    let mut files: Vec<std::path::PathBuf> = entries.flatten().map(|e| e.path()).collect();
    files.sort();
    for file in files {
        if file.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let body =
            std::fs::read_to_string(&file).map_err(|e| SkillError::io("read command", &e))?;
        if body.trim().is_empty() {
            report.skipped.push(file);
            continue;
        }
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = SkillName::sanitize(&stem);
        if name.as_str() != stem {
            report.lossy.push(LossyConversion {
                source: file.clone(),
                reason: format!("command name {stem:?} sanitized to {:?}", name.as_str()),
            });
        }
        let description = body
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim().trim_start_matches('#').trim().to_string())
            .unwrap_or_default();
        if description.is_empty() {
            report.lossy.push(LossyConversion {
                source: file.clone(),
                reason: "no description line found; empty description".to_string(),
            });
        }
        let dir = out_dir.join(name.as_str());
        std::fs::create_dir_all(&dir)
            .map_err(|e| SkillError::io("create command skill dir", &e))?;
        let doc = format!(
            "---\nname: {}\ndescription: {}\ntriggers: [\"/{}\"]\nversion: \"0.1.0\"\n---\n\n# /{}\n\n{}",
            name.as_str(),
            escape_yaml(&description),
            name.as_str(),
            name.as_str(),
            body
        );
        std::fs::write(dir.join("SKILL.md"), doc)
            .map_err(|e| SkillError::io("write command SKILL.md", &e))?;
        report.created.push(dir);
    }
    Ok(report)
}

fn escape_yaml(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\\\""))
}
