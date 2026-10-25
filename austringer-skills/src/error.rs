//! Typed skill-system errors: every failure mode is a variant, never a
//! panic, never a stringly-typed retry.

use std::path::PathBuf;

use crate::skill::SkillParseError;

/// Everything that can go wrong in the skills subsystem.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SkillError {
    /// A SKILL.md failed to parse. The scan continues; the error carries
    /// the offending path and line.
    #[error("skill parse error: {source}")]
    Parse {
        /// The typed parse error.
        source: SkillParseError,
    },
    /// A `{{shared:...}}` partial referenced by a skill body does not exist.
    #[error("shared partial {name:?} not found (looked in {path:?})")]
    SharedPartialNotFound {
        /// The missing partial name.
        name: String,
        /// The `_shared` directory that was searched.
        path: PathBuf,
    },
    /// A filesystem operation failed.
    #[error("io: {detail}")]
    Io {
        /// Underlying failure description.
        detail: String,
    },
    /// skills-lock.json exists but is not valid JSON or not the expected shape.
    #[error("skills-lock.json corrupt: {detail}")]
    LockCorrupt {
        /// What was wrong with the file.
        detail: String,
    },
    /// A findings-schema document failed schema validation.
    #[error("findings schema violations: {count}")]
    FindingsInvalid {
        /// Number of violations (details travel in the typed vector
        /// returned by [`crate::pack::validate_findings`]).
        count: usize,
    },
    /// A findings-schema file itself failed to parse.
    #[error("findings schema parse error: {detail}")]
    SchemaParse {
        /// What was wrong with the schema document.
        detail: String,
    },
}

impl SkillError {
    /// Wraps any std::io error with its path context.
    pub fn io(context: &str, err: &std::io::Error) -> Self {
        Self::Io {
            detail: format!("{context}: {err}"),
        }
    }
}

impl From<SkillParseError> for SkillError {
    fn from(source: SkillParseError) -> Self {
        Self::Parse { source }
    }
}
