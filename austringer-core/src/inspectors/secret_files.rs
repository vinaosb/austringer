//! Secret-file read guard and the transcript redactor.
//!
//! Reads of high-risk paths (`.env*`, `*id_rsa*`, `*.pem`, `*.p12`,
//! `*credentials*`, `.npmrc`, `.netrc`, keychain files — the list is
//! config-extensible) default to ASK. On an approved read the transcript
//! records a REDACTED placeholder while the executor still receives the
//! real path: masking applies to what enters the event log / LLM-visible
//! path, never to the execution argument. Secret values (file path,
//! content snippets) never enter LLM-visible events produced by this
//! module.

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::StageError;
use crate::inspectors::glob::Glob;
use crate::inspectors::permission::PATH_KEYS;
use crate::inspectors::verdict::{CallInspector, Verdict};

/// Default high-risk path patterns (glob syntax; matched against the full
/// normalized path *and* its basename).
pub const DEFAULT_SECRET_PATTERNS: &[&str] = &[
    ".env*",
    "*id_rsa*",
    "*.pem",
    "*.p12",
    "*credentials*",
    ".npmrc",
    ".netrc",
    "*keychain*",
];

/// Default tool-name globs treated as file readers.
pub const DEFAULT_READ_TOOLS: &[&str] = &["view_*", "read_*", "cat*"];

/// Redacts secret-looking values before they enter model-visible feedback
/// or the transcript.
#[derive(Debug, Clone)]
pub struct TranscriptRedactor {
    patterns: Vec<Glob>,
}

impl TranscriptRedactor {
    /// Builds a redactor from already-compiled patterns (the chain shares
    /// the secret-file guard's pattern set).
    #[must_use]
    pub fn from_globs(patterns: Vec<Glob>) -> Self {
        Self { patterns }
    }

    /// Builds a redactor from compiled patterns; invalid patterns are
    /// rejected (fail closed).
    pub fn try_new(patterns: &[String]) -> Result<Self, crate::inspectors::glob::InvalidGlob> {
        let mut compiled = Vec::with_capacity(patterns.len());
        for pattern in patterns {
            compiled.push(Glob::new(pattern)?);
        }
        Ok(Self { patterns: compiled })
    }

    /// Whether `path` (normalized, or its basename) matches any secret
    /// pattern. Basename matching is what makes `*credentials*` and
    /// `*.pem` fire on nested paths without requiring `**/` prefixes.
    #[must_use]
    pub fn is_secret_path(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        let basename = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
        self.patterns
            .iter()
            .any(|glob| glob.matches(&normalized) || glob.matches(basename))
    }

    /// Masks `path` when it is secret-looking; non-secret paths pass
    /// through untouched.
    #[must_use]
    pub fn mask_path(&self, path: &str) -> String {
        if self.is_secret_path(path) {
            "[REDACTED:secret-path]".to_owned()
        } else {
            path.to_owned()
        }
    }
}

impl Default for TranscriptRedactor {
    fn default() -> Self {
        // The constant defaults are compile-time literals pinned by tests;
        // a malformed entry would be a bug and is dropped (never widened).
        Self {
            patterns: DEFAULT_SECRET_PATTERNS
                .iter()
                .filter_map(|pattern| Glob::new(pattern).ok())
                .collect(),
        }
    }
}

/// Configuration for the secret-file guard.
#[derive(Debug, Clone)]
pub struct SecretFileConfig {
    /// Secret path patterns (matched on full path and basename).
    pub patterns: Vec<Glob>,
    /// Tool-name globs considered capable of reading files.
    pub read_tools: Vec<Glob>,
}

impl Default for SecretFileConfig {
    fn default() -> Self {
        Self {
            patterns: DEFAULT_SECRET_PATTERNS
                .iter()
                .filter_map(|pattern| Glob::new(pattern).ok())
                .collect(),
            read_tools: DEFAULT_READ_TOOLS
                .iter()
                .filter_map(|pattern| Glob::new(pattern).ok())
                .collect(),
        }
    }
}

/// The secret-file read guard: file-read calls whose path argument is
/// secret-looking default to ASK (subject to the chain's
/// authoritative-allow short-circuit; a Deny still wins).
#[derive(Debug, Clone)]
pub struct SecretFileInspector {
    config: SecretFileConfig,
}

impl SecretFileInspector {
    /// Builds the guard from `config`.
    #[must_use]
    pub fn new(config: SecretFileConfig) -> Self {
        Self { config }
    }

    fn is_secret_read(&self, call: &PendingCall) -> bool {
        if !self
            .config
            .read_tools
            .iter()
            .any(|glob| glob.matches(&call.tool))
        {
            return false;
        }
        let Some(object) = call.arguments.as_object() else {
            return false;
        };
        PATH_KEYS.iter().any(|key| {
            matches!(object.get(*key), Some(serde_json::Value::String(path))
            if !path.is_empty()
                && self.config.patterns.iter().any(|glob| {
                    let normalized = path.replace('\\', "/");
                    let basename =
                        normalized.rsplit('/').next().unwrap_or(normalized.as_str());
                    glob.matches(&normalized) || glob.matches(basename)
                }))
        })
    }
}

#[async_trait]
impl CallInspector for SecretFileInspector {
    fn name(&self) -> &'static str {
        "secret-files"
    }

    async fn inspect(&mut self, call: &PendingCall) -> Result<Verdict, StageError> {
        if self.is_secret_read(call) {
            Ok(Verdict::Ask {
                reason: "approval required: secret-file read (the file path is redacted here \
                         and will only be read if the user approves)"
                    .to_owned(),
                notice_on_approval: Some(
                    "secret-file read approved: the file path and any contents stay redacted \
                     from the transcript ([REDACTED])"
                        .to_owned(),
                ),
            })
        } else {
            Ok(Verdict::Allow {
                authoritative: false,
            })
        }
    }
}
