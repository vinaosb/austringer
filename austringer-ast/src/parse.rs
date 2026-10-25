//! Parsing and the syntax-validation surface: tree-sitter parse with
//! `is_error()`/`is_missing()` and position-exact, capped diagnostics.

use crate::error::AstError;
use crate::grammar;
use crate::language::LanguageId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Bounded-handling cap: sources above this size are rejected with
/// [`AstError::FileTooLarge`] instead of parsed (tree-sitter trees cost
/// tens of bytes per input byte; 64 MiB is far beyond any sane source
/// file and keeps worst-case memory bounded).
pub const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// Maximum diagnostics retained per parse; beyond this the list is
/// truncated and [`ParseResult::diagnostics_truncated`] reports it.
pub const MAX_DIAGNOSTICS: usize = 200;

/// What a diagnostic describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticKind {
    /// A tree-sitter `ERROR` node (unparseable region).
    SyntaxError,
    /// A tree-sitter `MISSING` node (expected-but-absent token).
    MissingNode,
}

/// One structured syntax diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// File the diagnostic came from; `None` for in-memory parses.
    pub file: Option<PathBuf>,
    /// Which kind of tree-sitter problem this is.
    pub kind: DiagnosticKind,
    /// Human-readable message (e.g. `missing "}"`).
    pub message: String,
    /// 1-based line.
    pub line: usize,
    /// 1-based byte column.
    pub column: usize,
    /// Byte offset of the region start.
    pub start_byte: usize,
    /// Byte offset of the region end.
    pub end_byte: usize,
}

/// The result of parsing one source file.
#[derive(Debug, Clone)]
pub struct ParseResult {
    language: LanguageId,
    source: Vec<u8>,
    tree: tree_sitter::Tree,
    diagnostics: Vec<Diagnostic>,
    diagnostics_truncated: bool,
    has_error_node: bool,
    has_missing: bool,
}

/// Parse `source` with `language`'s grammar.
///
/// Errors: [`AstError::FileTooLarge`] over the cap,
/// [`AstError::NoGrammarCompiled`] when the language's feature gate is off.
/// Syntactically broken input is NOT an error — inspect
/// [`ParseResult::is_error`] / [`ParseResult::diagnostics`].
pub fn parse(language: LanguageId, source: Vec<u8>) -> Result<ParseResult, AstError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(AstError::FileTooLarge {
            path: PathBuf::new(),
            size: source.len() as u64,
            max: MAX_SOURCE_BYTES as u64,
        });
    }
    let grammar = grammar::grammar_for(language).ok_or(AstError::NoGrammarCompiled { language })?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&grammar.language)
        .map_err(|e| AstError::LanguageSetup(e.to_string()))?;
    let tree = parser
        .parse(&source, None)
        .ok_or(AstError::ParseCanceled(PathBuf::new()))?;
    let (diagnostics, truncated, has_error_node, has_missing) = collect_diagnostics(&tree);
    Ok(ParseResult {
        language,
        source,
        tree,
        diagnostics,
        diagnostics_truncated: truncated,
        has_error_node,
        has_missing,
    })
}

/// Iterative (stack-safe on adversarially deep trees) diagnostic walk:
/// topmost `ERROR` regions and every `MISSING` node become diagnostics in
/// position order, capped at [`MAX_DIAGNOSTICS`]. Children of an `ERROR`
/// node are skipped — sub-errors inside an error region are artifacts.
fn collect_diagnostics(tree: &tree_sitter::Tree) -> (Vec<Diagnostic>, bool, bool, bool) {
    let mut diagnostics = Vec::new();
    let mut truncated = false;
    let mut has_error_node = false;
    let mut has_missing = false;
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            has_error_node |= node.is_error();
            has_missing |= node.is_missing();
            if diagnostics.len() == MAX_DIAGNOSTICS {
                truncated = true;
                continue;
            }
            let kind = if node.is_error() {
                DiagnosticKind::SyntaxError
            } else {
                DiagnosticKind::MissingNode
            };
            let start = node.start_position();
            diagnostics.push(Diagnostic {
                file: None,
                kind,
                message: if node.is_missing() {
                    format!("missing \"{}\"", node.kind())
                } else {
                    "syntax error".to_owned()
                },
                line: start.row + 1,
                column: start.column + 1,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
            continue;
        }
        for index in (0..node.child_count()).rev() {
            if let Some(child) = node.child(index) {
                stack.push(child);
            }
        }
    }
    diagnostics.sort_by_key(|d| d.start_byte);
    (diagnostics, truncated, has_error_node, has_missing)
}

impl ParseResult {
    /// Language the source was parsed as.
    pub fn language(&self) -> LanguageId {
        self.language
    }

    /// The raw source bytes (tree-sitter is byte-oriented; lossy-convert
    /// when displaying).
    pub fn source(&self) -> &[u8] {
        &self.source
    }

    /// Whether the tree contains any `ERROR` node (a region the parser
    /// could not attach — distinct from [`ParseResult::is_missing`], which
    /// reports expected-but-absent tokens).
    pub fn is_error(&self) -> bool {
        self.has_error_node
    }

    /// Whether the tree contains any `MISSING` node.
    pub fn is_missing(&self) -> bool {
        self.has_missing
    }

    /// Structured diagnostics, position-ordered, capped at
    /// [`MAX_DIAGNOSTICS`].
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether diagnostics were dropped because of the cap.
    pub fn diagnostics_truncated(&self) -> bool {
        self.diagnostics_truncated
    }

    /// Internal: the tree, for tag extraction and diagnostics.
    pub(crate) fn tree(&self) -> &tree_sitter::Tree {
        &self.tree
    }
}
