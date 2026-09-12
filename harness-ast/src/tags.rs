//! Symbol-table extraction: runs the per-language tags query
//! (`name.definition.<kind>` / `name.reference.<kind>` capture convention,
//! `@_span` optionally capturing the definition container) over a parse
//! tree and produces ordered definitions + references.
//!
//! Engine choice (justification): tree-sitter's own `Query`/`QueryCursor`
//! over the facade we already depend on, instead of the `tree-sitter-tags`
//! crate — that crate pins its own facade version, returns names through a
//! drain-buffer we would immediately re-extract into serde types anyway,
//! and its API hardcodes the old `@definition.X` + `@name` pairing. The
//! ~150 lines below own the convention outright and hand the cache
//! serializable [`Definition`]/[`Reference`] values directly.

use crate::error::AstError;
use crate::grammar;
use crate::parse::ParseResult;
use serde::{Deserialize, Serialize};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

/// Match-limit guard for pathological query backtracking; extraction
/// reports [`TagTable::truncated`] when exceeded.
pub const TAG_MATCH_LIMIT: u32 = 100_000;

const DEF_PREFIX: &str = "name.definition.";
const REF_PREFIX: &str = "name.reference.";
const CONTAINER: &str = "_span";

/// A 0-based byte position (tree-sitter convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    /// 0-based line.
    pub row: u32,
    /// 0-based byte column.
    pub column: u32,
}

/// A half-open byte span with its point coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of the first byte.
    pub start_byte: usize,
    /// Byte offset one past the last byte.
    pub end_byte: usize,
    /// [`Point`] of `start_byte`.
    pub start: Point,
    /// [`Point`] of `end_byte`.
    pub end: Point,
}

/// What a symbol is. `Other` carries language-specific kinds verbatim
/// (e.g. verilog `macro`, elm `port`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SymbolKind {
    /// A standalone function.
    Function,
    /// A type-attached function.
    Method,
    /// A struct definition.
    Struct,
    /// A class definition.
    Class,
    /// An enum definition.
    Enum,
    /// One enum case.
    EnumMember,
    /// A trait/protocol.
    Interface,
    /// A module/namespace.
    Module,
    /// A type alias or named type.
    Type,
    /// A constant or static.
    Constant,
    /// A variable or binding.
    Variable,
    /// An object property or field.
    Property,
    /// A document/config section heading.
    Section,
    /// A macro definition.
    Macro,
    /// A call/reference site.
    Call,
    /// Anything language-specific.
    Other(String),
}

impl SymbolKind {
    /// Parse the `<kind>` suffix out of a `name.definition.<kind>` capture
    /// name.
    pub fn from_suffix(suffix: &str) -> Self {
        match suffix {
            "function" => SymbolKind::Function,
            "method" => SymbolKind::Method,
            "macro" => SymbolKind::Macro,
            "struct" => SymbolKind::Struct,
            "class" => SymbolKind::Class,
            "enum" => SymbolKind::Enum,
            "enumMember" => SymbolKind::EnumMember,
            "interface" => SymbolKind::Interface,
            "module" => SymbolKind::Module,
            "type" => SymbolKind::Type,
            "constant" => SymbolKind::Constant,
            "variable" => SymbolKind::Variable,
            "property" => SymbolKind::Property,
            "section" => SymbolKind::Section,
            "call" => SymbolKind::Call,
            other => SymbolKind::Other(other.to_owned()),
        }
    }
}

/// A defined symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    /// Lossy-UTF-8 symbol name.
    pub name: String,
    /// What kind of symbol.
    pub kind: SymbolKind,
    /// Span of exactly the name token.
    pub name_span: Span,
    /// Span of the containing definition item, when the query captured
    /// `@_span`.
    pub def_span: Option<Span>,
}

/// A reference site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// Lossy-UTF-8 referenced name.
    pub name: String,
    /// Reference kind (typically [`SymbolKind::Call`]).
    pub kind: SymbolKind,
    /// Span of exactly the name token.
    pub span: Span,
}

/// The extracted symbol table for one file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagTable {
    /// Definitions, position-ordered.
    pub definitions: Vec<Definition>,
    /// References, position-ordered.
    pub references: Vec<Reference>,
    /// Whether the query match limit was hit (results may be partial).
    pub truncated: bool,
}

fn span_of(node: tree_sitter::Node) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    Span {
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start: Point {
            row: start.row as u32,
            column: start.column as u32,
        },
        end: Point {
            row: end.row as u32,
            column: end.column as u32,
        },
    }
}

fn name_at(source: &[u8], node: tree_sitter::Node) -> String {
    String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()]).into_owned()
}

/// Extract the symbol table from an already-parsed source.
///
/// Errors: [`AstError::NoGrammarCompiled`] (feature off) or
/// [`AstError::TagsQuery`] (a shipped query failed to compile — a harness
/// bug). Broken syntax is NOT an error: tags come from whatever parsed.
pub fn extract(parsed: &ParseResult) -> Result<TagTable, AstError> {
    let language = parsed.language();
    let grammar = grammar::grammar_for(language).ok_or(AstError::NoGrammarCompiled { language })?;
    let query =
        Query::new(&grammar.language, grammar.tags_query).map_err(|e| AstError::TagsQuery {
            language: language.name(),
            query_error: e.to_string(),
        })?;
    let capture_names = query.capture_names();
    let source = parsed.source();

    let mut definitions = Vec::new();
    let mut references = Vec::new();
    let mut cursor = QueryCursor::new();
    cursor.set_match_limit(TAG_MATCH_LIMIT);
    let mut matches = cursor.matches(&query, parsed.tree().root_node(), source);
    while let Some(query_match) = matches.next() {
        let mut container = None;
        for capture in query_match.captures() {
            if capture_names[capture.index as usize] == CONTAINER {
                container = Some(span_of(capture.node));
            }
        }
        for capture in query_match.captures() {
            let capture_name = capture_names[capture.index as usize];
            if let Some(kind) = capture_name.strip_prefix(DEF_PREFIX) {
                definitions.push(Definition {
                    name: name_at(source, capture.node),
                    kind: SymbolKind::from_suffix(kind),
                    name_span: span_of(capture.node),
                    def_span: container,
                });
            } else if let Some(kind) = capture_name.strip_prefix(REF_PREFIX) {
                references.push(Reference {
                    name: name_at(source, capture.node),
                    kind: SymbolKind::from_suffix(kind),
                    span: span_of(capture.node),
                });
            }
        }
    }
    drop(matches);
    let truncated = cursor.did_exceed_match_limit();
    definitions.sort_by_key(|d| d.name_span.start_byte);
    references.sort_by_key(|r| r.span.start_byte);
    Ok(TagTable {
        definitions,
        references,
        truncated,
    })
}
