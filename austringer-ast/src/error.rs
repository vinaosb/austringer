//! Typed errors for the parse/tag engine. Every failure mode reachable
//! from the public API maps to exactly one variant.

use crate::language::LanguageId;
use std::path::PathBuf;
use thiserror::Error;

/// All fallible `austringer-ast` operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AstError {
    /// Underlying filesystem failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Source exceeded the bounded-handling cap ([`crate::parse::MAX_SOURCE_BYTES`]).
    #[error("file {path} is {size} bytes, over the {max}-byte parse cap")]
    FileTooLarge {
        /// Path as reported by the caller (empty for in-memory sources).
        path: PathBuf,
        /// Actual size in bytes.
        size: u64,
        /// The cap in bytes.
        max: u64,
    },

    /// The language's grammar feature was not compiled into this build.
    /// Path-level analysis degrades to
    /// [`crate::analyze::AnalyzeOutcome::NoGrammar`] instead of this error.
    #[error("no grammar compiled in for {language:?} (feature {} is off)", language.feature_gate())]
    NoGrammarCompiled {
        /// The language whose grammar crate is absent.
        language: LanguageId,
    },

    /// tree-sitter rejected the language ABI — a build/config bug, not a
    /// user state; carries the engine's message.
    #[error("language setup failed: {0}")]
    LanguageSetup(String),

    /// The parser bailed out (only possible with cancellation callbacks,
    /// kept typed so no path can invent a `None` unwrap).
    #[error("parser returned no tree for {0}")]
    ParseCanceled(PathBuf),

    /// A per-language tags query failed to compile — a austringer bug (queries
    /// ship with the crate); carries language and the query error.
    #[error("tags query failed to compile for {language}: {query_error}")]
    TagsQuery {
        /// Language whose embedded query is broken.
        language: &'static str,
        /// Underlying query-compiler message.
        query_error: String,
    },
}
