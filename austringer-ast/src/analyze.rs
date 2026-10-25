//! The orchestrator: path → language → cache → parse → tags →
//! diagnostics, with the no-grammar degradation contract.

use crate::cache::{FileStamp, TableCache};
use crate::error::AstError;
use crate::language::LanguageId;
use crate::parse::{self, Diagnostic};
use crate::tags;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How an analysis interacted with the persistent cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheStatus {
    /// Analyzer runs without a cache dir.
    Disabled,
    /// No usable entry: the file was parsed and the entry (re)written.
    Miss,
    /// Content hash + length matched a current-version entry: no parse.
    Hit,
    /// Cache hit/miss decided, but persisting the entry failed (analysis
    /// results are still valid; the cache is an accelerator, not truth).
    StoreFailed,
}

/// Symbol tables + diagnostics for one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Analysis {
    /// Language the file was analyzed as.
    pub language: LanguageId,
    /// Definitions, position-ordered (see [`crate::tags`]).
    pub definitions: Vec<tags::Definition>,
    /// References, position-ordered.
    pub references: Vec<tags::Reference>,
    /// Structured syntax diagnostics with file/line/col.
    pub diagnostics: Vec<Diagnostic>,
    /// Any `ERROR` region present (see [`crate::parse::ParseResult::is_error`]).
    pub has_error: bool,
    /// Any `MISSING` node present.
    pub has_missing: bool,
    /// Tags query hit its match limit (tables may be partial).
    pub tags_truncated: bool,
    /// Cache interaction for this call.
    pub cache: CacheStatus,
}

/// Result of path-level analysis. Anything without a grammar — unknown
/// extension, or a known language whose grammar feature is not compiled
/// in — degrades to [`AnalyzeOutcome::NoGrammar`] so pipelines fall back
/// to text tools instead of failing.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalyzeOutcome {
    /// Analysis succeeded (possibly with syntax diagnostics inside).
    Analyzed(Box<Analysis>),
    /// No grammar available. `language` is `Some` when the extension is
    /// known but the grammar feature is off, `None` for unknown types.
    NoGrammar {
        /// The path that was rejected.
        path: PathBuf,
        /// The detected language, when the extension was recognized.
        language: Option<LanguageId>,
    },
}

/// Path-level analyzer with an optional persistent table cache.
#[derive(Clone, Default)]
pub struct Analyzer {
    cache: Option<TableCache>,
}

impl Analyzer {
    /// A cacheless analyzer (every call parses).
    pub fn new() -> Self {
        Analyzer { cache: None }
    }

    /// An analyzer persisting tables under `dir` (created on first store;
    /// caller-chosen location, e.g. `<project>/.austringer/ast-cache`).
    pub fn with_cache_dir(dir: impl Into<PathBuf>) -> Self {
        Analyzer {
            cache: Some(TableCache::new(dir.into())),
        }
    }

    /// Analyze a file on disk. Reads bytes, detects the language, consults
    /// the cache, and parses + extracts tags on miss. IO failures and the
    /// size cap are [`AstError`]s; missing grammars are not — they are the
    /// typed [`AnalyzeOutcome::NoGrammar`] degrade.
    pub fn analyze_path(&self, path: &Path) -> Result<AnalyzeOutcome, AstError> {
        let language = match LanguageId::from_path(path) {
            Some(language) if crate::grammar::grammar_for(language).is_some() => language,
            Some(language) => {
                return Ok(AnalyzeOutcome::NoGrammar {
                    path: path.to_path_buf(),
                    language: Some(language),
                });
            }
            None => {
                return Ok(AnalyzeOutcome::NoGrammar {
                    path: path.to_path_buf(),
                    language: None,
                });
            }
        };
        let metadata = std::fs::metadata(path)?;
        if metadata.len() > parse::MAX_SOURCE_BYTES as u64 {
            return Err(AstError::FileTooLarge {
                path: path.to_path_buf(),
                size: metadata.len(),
                max: parse::MAX_SOURCE_BYTES as u64,
            });
        }
        let source = std::fs::read(path)?;
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let stamp = FileStamp {
            mtime: metadata.modified()?,
            len: metadata.len(),
        };
        let content_hash = blake3::hash(&source);

        if let Some((hit, stale_mtime)) = self
            .cache
            .as_ref()
            .and_then(|cache| cache.load(&canonical, &content_hash, &stamp))
        {
            let mut analysis = hit;
            for diagnostic in &mut analysis.diagnostics {
                diagnostic.file = Some(path.to_path_buf());
            }
            if stale_mtime {
                let _ = self.cache.as_ref().map(|cache| {
                    cache.store(&canonical, &stamp, &content_hash, language, &analysis)
                });
            }
            return Ok(AnalyzeOutcome::Analyzed(Box::new(analysis)));
        }

        let mut analysis = self.analyze_bytes(language, source)?;
        analysis.cache = match &self.cache {
            None => CacheStatus::Disabled,
            Some(cache) => {
                match cache.store(&canonical, &stamp, &content_hash, language, &analysis) {
                    Ok(()) => CacheStatus::Miss,
                    Err(_) => CacheStatus::StoreFailed,
                }
            }
        };
        for diagnostic in &mut analysis.diagnostics {
            diagnostic.file = Some(path.to_path_buf());
        }
        Ok(AnalyzeOutcome::Analyzed(Box::new(analysis)))
    }

    /// Analyze in-memory bytes (no cache interaction; the parse cap
    /// applies). Errors only for typed conditions — never for broken
    /// syntax, which lands in `diagnostics`.
    pub fn analyze_bytes(
        &self,
        language: LanguageId,
        source: Vec<u8>,
    ) -> Result<Analysis, AstError> {
        let parsed = parse::parse(language, source)?;
        let table = tags::extract(&parsed)?;
        Ok(Analysis {
            language,
            definitions: table.definitions,
            references: table.references,
            diagnostics: parsed.diagnostics().to_vec(),
            has_error: parsed.is_error(),
            has_missing: parsed.is_missing(),
            tags_truncated: table.truncated,
            cache: CacheStatus::Disabled,
        })
    }
}
