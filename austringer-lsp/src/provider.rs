//! The provider seam (IDE-as-toolbox, JetBrains §24): navigation questions
//! answered by *some* semantic backend. Two implementations exist:
//!
//! * [`crate::router::LspRouter`] — the native, self-spawned server client
//!   (this todo).
//! * [`IdeBridgeStub`] — a future VSCode-extension bridge exposing the
//!   host IDE's real semantic index over a localhost MCP endpoint
//!   (registered but unimplemented; plan todo 38 wires it).
//!
//! Routing priority is bridge > native: when a bridge covers a language,
//! the router never spawns that language's server (tested, not hoped).

use std::path::PathBuf;

use async_trait::async_trait;

use crate::config::Language;
use crate::error::{DegradeReason, LspError};

/// Machine-readable hint carried by [`Answer::DegradedToSearch`] — the
/// fallback seam points at plain lexical search (grep-style) without this
/// crate depending on any search implementation.
pub const SEARCH_FALLBACK_HINT: &str = "LSP degraded: retry navigation with lexical search";

/// Where a navigation question is asked: 1-based line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPosition {
    /// Absolute path of the document.
    pub file: PathBuf,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub column: u32,
}

/// A navigation target: 1-based line/column in a local file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Absolute path of the target.
    pub file: PathBuf,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub column: u32,
}

/// Bounded hover text (see [`crate::render`] for the budget philosophy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverText(String);

impl HoverText {
    /// Wraps `text`, truncating to the hover budget at a char boundary.
    #[must_use]
    pub fn new(text: &str) -> Self {
        const HOVER_BUDGET_CHARS: usize = 480;
        if text.chars().count() <= HOVER_BUDGET_CHARS {
            return Self(text.to_owned());
        }
        let mut truncated: String = text.chars().take(HOVER_BUDGET_CHARS - 1).collect();
        truncated.push('…');
        Self(truncated)
    }

    /// The bounded text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A typed navigation answer. Degradation is a *result*, not an error: the
/// caller learns that semantic navigation is unavailable and is pointed at
/// the lexical-search fallback via [`SEARCH_FALLBACK_HINT`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Navigation locations (possibly empty — "no definition found" is a
    /// valid answer, not a failure). Bounded to
    /// [`crate::render::MAX_LOCATIONS`].
    Locations(Vec<Location>),
    /// Bounded hover text.
    Hover(HoverText),
    /// Semantic navigation is off for this language; use lexical search.
    DegradedToSearch {
        /// Why.
        reason: DegradeReason,
        /// The fallback seam (see [`SEARCH_FALLBACK_HINT`]).
        hint: &'static str,
    },
}

/// A semantic navigation backend. The router and the future IDE bridge
/// both implement this; the tool layer (austringer-core) talks only to it.
#[async_trait]
pub trait LspProvider: Send + Sync {
    /// Languages this provider can serve. Drives bridge-priority routing.
    fn languages(&self) -> &[Language];

    /// Definitions of the symbol at `position`.
    async fn goto_definition(&self, position: DocumentPosition) -> Result<Answer, LspError>;

    /// References to the symbol at `position`.
    async fn find_references(&self, position: DocumentPosition) -> Result<Answer, LspError>;

    /// Hover/signature text for the symbol at `position`.
    async fn hover(&self, position: DocumentPosition) -> Result<Answer, LspError>;
}

/// Bridge-shaped stub for the IDE-as-toolbox endpoint (plan todo 38).
///
/// Registered like any provider (so priority routing and the "no duplicate
/// servers" rule can be exercised today), but its navigation methods
/// return [`LspError::BridgeNotWired`] until the localhost MCP endpoint
/// exists. The endpoint string is stored so configuration shape is final.
#[derive(Debug, Clone)]
pub struct IdeBridgeStub {
    endpoint: String,
    languages: Vec<Language>,
}

impl IdeBridgeStub {
    /// Creates the stub for a bridge endpoint (e.g. `http://127.0.0.1:<port>`)
    /// covering `languages`.
    #[must_use]
    pub fn new(endpoint: impl Into<String>, languages: &[Language]) -> Self {
        Self {
            endpoint: endpoint.into(),
            languages: languages.to_vec(),
        }
    }

    /// The configured bridge endpoint.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[async_trait]
impl LspProvider for IdeBridgeStub {
    fn languages(&self) -> &[Language] {
        &self.languages
    }

    async fn goto_definition(&self, _position: DocumentPosition) -> Result<Answer, LspError> {
        Err(LspError::BridgeNotWired)
    }

    async fn find_references(&self, _position: DocumentPosition) -> Result<Answer, LspError> {
        Err(LspError::BridgeNotWired)
    }

    async fn hover(&self, _position: DocumentPosition) -> Result<Answer, LspError> {
        Err(LspError::BridgeNotWired)
    }
}
