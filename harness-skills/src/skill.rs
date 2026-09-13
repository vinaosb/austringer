//! SKILL.md parsing: agentskills.io-compatible frontmatter plus harness
//! extensions (triggers incl. non-English phrases, negative scoping,
//! languages, version, allowed-tools, per-model guidance, context_fork).
//!
//! Strict on structure (malformed YAML shape = typed error with path and
//! line), tolerant on content (unknown keys = typed warnings, missing
//! name/description are derived with warnings — matching how opencode
//! loads bare-markdown skills, which the real corpus contains).
//! Bodies are untrusted data: nothing here interprets or executes them.

use std::path::PathBuf;

pub use crate::parse::parse_skill_document;

/// Matches a tool name against an allowed-tools pattern where `*` is a
/// wildcard (e.g. `Bash(git:*)` matches `Bash(git:status)`). Reimplemented
/// here instead of depending on harness-core's glob engine because core is
/// under active compaction (todo 13) and this needs only `*` semantics.
pub fn tool_pattern_matches(pattern: &str, tool: &str) -> bool {
    let mut tool_rest = tool;
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return false;
    };
    if !tool_rest.starts_with(first) {
        return false;
    }
    tool_rest = &tool_rest[first.len()..];
    for part in parts {
        if part.is_empty() {
            continue;
        }
        match tool_rest.find(part) {
            Some(idx) => tool_rest = &tool_rest[idx + part.len()..],
            None => return false,
        }
    }
    true
}

/// agentskills.io name shape: lowercase kebab, max 64 chars.
pub const NAME_MAX: usize = 64;
/// agentskills.io description cap.
pub const DESCRIPTION_MAX: usize = 1024;

/// Validated skill name (lowercase kebab).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct SkillName(String);

impl SkillName {
    /// Validates kebab shape; `None` when invalid.
    pub fn new(raw: &str) -> Option<Self> {
        let ok = !raw.is_empty()
            && raw.len() <= NAME_MAX
            && raw
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !raw.starts_with('-')
            && !raw.ends_with('-')
            && !raw.contains("--");
        ok.then(|| Self(raw.to_string()))
    }

    /// Sanitizes to kebab (lowercase, non-alnum runs collapse to `-`).
    pub fn sanitize(raw: &str) -> Self {
        let mut out = String::new();
        let mut dash = false;
        for ch in raw.chars() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch.to_ascii_lowercase());
                dash = false;
            } else if !dash && !out.is_empty() {
                out.push('-');
                dash = true;
            }
        }
        let trimmed = out.trim_end_matches('-');
        if trimmed.is_empty() {
            Self("skill".to_string())
        } else {
            Self(trimmed.chars().take(NAME_MAX).collect())
        }
    }

    /// The name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SkillName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Per-model guidance entry (order preserved from the frontmatter).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ModelGuidance {
    /// Model identifier the guidance targets (e.g. `glm-5`).
    pub model: String,
    /// The guidance text.
    pub guidance: String,
}

/// Routing metadata surface ONLY: bodies are never part of this type
/// (progressive disclosure — bodies load through
/// [`crate::loader::SkillIndex::invoke`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SkillMeta {
    /// Validated skill name.
    pub name: SkillName,
    /// agentskills.io description (≤1024 chars when author-declared).
    pub description: String,
    /// Declared version, if any.
    pub version: Option<String>,
    /// Declared languages.
    pub languages: Vec<String>,
    /// Explicit trigger phrases (may be non-English; verbatim).
    pub triggers: Vec<String>,
    /// Negative-scoping phrases: matching these blocks routing.
    pub negative_triggers: Vec<String>,
    /// Declared allowed tools (may carry `*` wildcards, e.g. `Bash(git:*)`).
    pub allowed_tools: Vec<String>,
    /// Per-model guidance sections.
    pub model_guidance: Vec<ModelGuidance>,
    /// `context_fork: true` declaration consumed by the orchestration layer.
    pub context_fork: bool,
    /// Vendor dir the skill was discovered in (loader-filled).
    pub vendor: crate::loader::Vendor,
    /// Monorepo subtree this skill is scoped to (loader-filled).
    pub scope: Option<PathBuf>,
    /// Skill directory (loader-filled; used by invoke).
    pub skill_dir: PathBuf,
}

/// Typed parse failure carrying the file and line.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{}:{}: {}", path.display(), line, kind)]
pub struct SkillParseError {
    /// Offending file.
    pub path: PathBuf,
    /// 1-based line (or EOF position for unterminated blocks).
    pub line: u32,
    /// What went wrong.
    pub kind: ParseErrorKind,
}

/// Parse error taxonomy.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// Frontmatter opened but never closed with `---`.
    #[error("unterminated frontmatter")]
    UnterminatedFrontmatter,
    /// Frontmatter content is not a `key: value` mapping.
    #[error("not a mapping: {0}")]
    NotAMapping(String),
    /// Duplicate top-level key.
    #[error("duplicate key {0:?}")]
    DuplicateKey(String),
    /// Explicitly declared name violates the kebab shape.
    #[error("invalid name {0:?}: expected lowercase kebab (a-z0-9-), max {NAME_MAX}")]
    InvalidName(String),
    /// A required-shape field exceeded its agentskills.io cap.
    #[error("field {field:?} too long: {len} > {max}")]
    FieldTooLong {
        /// Field name.
        field: &'static str,
        /// Actual length.
        len: usize,
        /// Cap.
        max: usize,
    },
    /// Malformed flow sequence.
    #[error("{0}")]
    BadFlowSeq(String),
    /// The SKILL.md file could not be read.
    #[error("unreadable: {0}")]
    ReadFailed(String),
}

/// Typed, non-fatal parse warning.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillWarning {
    /// File the warning came from.
    pub path: PathBuf,
    /// 1-based line (0 when not line-specific).
    pub line: u32,
    /// What was tolerated.
    pub kind: WarningKind,
}

impl SkillWarning {
    /// True when the file carried no frontmatter at all (derived name).
    pub fn is_missing_frontmatter(&self) -> bool {
        matches!(self.kind, WarningKind::MissingFrontmatter)
    }

    /// True when an unknown (forward-compat) key was tolerated.
    pub fn is_unknown_key(&self) -> bool {
        matches!(self.kind, WarningKind::UnknownKey { .. })
    }

    /// True when invalid UTF-8 was replaced lossily.
    pub fn is_invalid_utf8(&self) -> bool {
        matches!(self.kind, WarningKind::InvalidUtf8 { .. })
    }
}

/// Warning taxonomy.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum WarningKind {
    /// No `---` frontmatter; name derived from the directory, description
    /// from the first `#` heading.
    #[error("missing frontmatter: name derived from directory")]
    MissingFrontmatter,
    /// `name` key absent; derived from the directory name.
    #[error("missing name: derived from directory")]
    DerivedName,
    /// `description` key absent; derived from the first heading or empty.
    #[error("missing description: derived from first heading")]
    DerivedDescription,
    /// Unknown frontmatter key tolerated for forward compatibility.
    #[error("unknown key {key:?} ignored")]
    UnknownKey {
        /// The tolerated key.
        key: String,
    },
    /// Invalid UTF-8 bytes replaced with U+FFFD.
    #[error("{replaced} invalid UTF-8 byte(s) replaced lossily")]
    InvalidUtf8 {
        /// Replacement chars emitted.
        replaced: usize,
    },
    /// Description exceeded the agentskills.io 1024-char cap and was
    /// truncated for the metadata surface (the real corpus contains such
    /// skills; compat-swallow posture keeps them loadable).
    #[error("description truncated from {original_len} to 1024 chars")]
    DescriptionTruncated {
        /// Original length before truncation.
        original_len: usize,
    },
    /// `context_fork` was not `true`/`false`; treated as false.
    #[error("context_fork value not boolean: treated as false")]
    ContextForkNotBool,
}
