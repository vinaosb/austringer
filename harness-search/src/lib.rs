//! Lexical code search: a Tantivy BM25 index plus ripgrep-family streaming
//! regex search, sharing one gitignore-respecting file walk.
//!
//! Guardrail (plan Scope-OUT): **no embeddings, no vectors, no semantic
//! anything** — this crate is strictly lexical (BM25 + regex) by design.
//!
//! # Layout
//! - [`CodeIndex`] — BM25 index, incremental mtime+size refresh, definition-
//!   boosted ranking, bounded snippets. Index location is caller-chosen
//!   (e.g. `<session-or-project>/.harness/index`).
//! - [`grep_stream`] — lazy regex search with windowed, deduped, capped
//!   results over the same ignore rules.
//!
//! # Concurrency
//! Concurrent opens are safe: tantivy's directory write lock (`meta.lock`,
//! an OS advisory lock held per refresh) rejects a second writer with
//! [`SearchError::IndexLocked`], while any number of readers search the
//! last committed snapshot lock-free. See `index` module docs.
//!
//! # Binary safety
//! Files with a NUL byte in the leading sniff window are never indexed and
//! quit the grep searcher early; invalid UTF-8 is handled lossily. Neither
//! path panics or pollutes results.

mod collector;
mod defs;
mod error;
mod grep;
mod index;
mod options;
mod schema;
mod search;
mod walk;

pub use error::SearchError;
pub use grep::{GrepHit, grep_stream};
pub use index::{CodeIndex, RefreshReport};
pub use options::{GrepOptions, IndexOptions};
pub use search::{DEFINITION_BOOST, SNIPPET_MAX_CHARS, SearchHit};
