//! Typed errors for lexical search. Every failure mode reachable from the
//! public API maps to exactly one variant; internal engine errors carry
//! their `Display` text.

use thiserror::Error;

/// All fallible `austringer-search` operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SearchError {
    /// Another writer holds the index directory lock (tantivy `meta.lock`).
    #[error("index directory lock held by another writer: {0}")]
    IndexLocked(String),

    /// The directory holds a tantivy index created with a different schema.
    #[error("incompatible index schema at {0}")]
    IncompatibleIndex(String),

    /// The BM25 query string is not valid query-grammar.
    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// The grep pattern is not a valid regex.
    #[error("invalid regex: {0}")]
    InvalidRegex(String),

    /// Underlying storage or filesystem failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Tantivy-internal failure not otherwise classified.
    #[error("index engine: {0}")]
    Engine(String),
}
