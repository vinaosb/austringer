//! Definitions-only read mode (plan todo 16, clause 6): tree-sitter
//! name+kind+line rows for cheap file orientation, instead of loading a
//! window. Unknown extensions and un-compiled grammars degrade to
//! [`OutlineOutcome::NoGrammar`] (the harness-ast contract — callers fall
//! back to windows).

use crate::error::ToolsError;
use crate::lines::LineTable;
use harness_ast::tags::SymbolKind;
use harness_ast::{Analysis, Analyzer, AstError, LanguageId};
use std::path::Path;

/// The outcome of an outline request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutlineOutcome {
    /// Rendered definition rows.
    Outline(OutlineView),
    /// No grammar for this path; fall back to windowed reads.
    NoGrammar {
        /// The path that was rejected.
        path: std::path::PathBuf,
    },
}

/// The rendered outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineView {
    /// One row per definition: `{line:>6} {kind:<9} {name}`.
    pub rows: Vec<String>,
    /// All rows joined by newlines.
    pub render: String,
}

/// Short stable label per symbol kind (shared with hunk grouping).
#[must_use]
pub fn kind_label(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function | SymbolKind::Macro => "fn",
        SymbolKind::Method => "method",
        SymbolKind::Struct => "struct",
        SymbolKind::Class => "class",
        SymbolKind::Enum => "enum",
        SymbolKind::EnumMember => "enumitem",
        SymbolKind::Interface => "trait",
        SymbolKind::Module => "mod",
        SymbolKind::Type => "type",
        SymbolKind::Constant => "const",
        SymbolKind::Variable => "let",
        SymbolKind::Property => "field",
        SymbolKind::Section => "section",
        SymbolKind::Call => "call",
        SymbolKind::Other(_) => "sym",
        _ => "sym",
    }
}

fn analyze(language: LanguageId, bytes: Vec<u8>) -> Result<Analysis, Option<ToolsError>> {
    match Analyzer::new().analyze_bytes(language, bytes) {
        Ok(analysis) => Ok(analysis),
        Err(AstError::NoGrammarCompiled { .. }) => Err(None),
        Err(other) => Err(Some(ToolsError::GateEngine(other))),
    }
}

/// Produce the definitions-only outline of `source` at `path`.
///
/// Errors: [`ToolsError::NonUtf8`], [`ToolsError::GateEngine`].
pub fn outline(path: &Path, source: &[u8]) -> Result<OutlineOutcome, ToolsError> {
    let table = LineTable::from_bytes(source.to_vec()).map_err(|_| ToolsError::NonUtf8 {
        path: path.to_path_buf(),
    })?;
    let Some(language) = LanguageId::from_path(path) else {
        return Ok(OutlineOutcome::NoGrammar {
            path: path.to_path_buf(),
        });
    };
    let analysis = match analyze(language, source.to_vec()) {
        Ok(analysis) => analysis,
        Err(None) => {
            return Ok(OutlineOutcome::NoGrammar {
                path: path.to_path_buf(),
            });
        }
        Err(Some(error)) => return Err(error),
    };
    let mut rows = Vec::with_capacity(analysis.definitions.len());
    for definition in &analysis.definitions {
        let line = table.line_of_byte(definition.name_span.start_byte);
        rows.push(format!(
            "{line:>6} {:<9} {}",
            kind_label(&definition.kind),
            definition.name
        ));
    }
    let render = rows.join("\n");
    Ok(OutlineOutcome::Outline(OutlineView { rows, render }))
}
