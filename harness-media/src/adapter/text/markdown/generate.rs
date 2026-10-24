//! The markdown writer: spec -> comrak AST -> comrak's canonical
//! CommonMark formatter. Rendering through comrak (not a hand-rolled
//! emitter) is what pins the dialect: one canonical spelling for every
//! construct, escaped where needed, byte-deterministic (markdown has no
//! clock fields). The md->html projection rides the same parse with the
//! html formatter.

use comrak::nodes::{
    AstNode, NodeCodeBlock, NodeHeading, NodeList, NodeTable, NodeValue, TableAlignment,
};
use comrak::{Arena, Options, format_commonmark, format_html, parse_document};

use super::spec::{Block, MdSpec};
use crate::error::MediaError;
use crate::extract::ExtractBudgets;
use crate::generate::GenerationSpec;

/// The shared parse/render option set: CommonMark 0.31.2 + the GFM
/// extension family (table, strikethrough, autolink, tasklist - the
/// AST-bearing subset; comrak's render-only `tagfilter` is deprecated
/// upstream and deliberately not enabled). Extraction, generation, and
/// the html projection all use the SAME dialect - the census and the
/// writer can never disagree.
#[must_use]
pub fn gfm_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options
}

/// Generate deterministic canonical-markdown bytes from a validated
/// spec.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec does not satisfy the
/// markdown contract (format mismatch, malformed params, semantic
/// violations - see [`MdSpec::from_generation`]).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let parsed = MdSpec::from_generation(spec)?;
    let arena = Arena::new();
    let root = arena.alloc(AstNode::from(NodeValue::Document));
    for block in &parsed.blocks {
        append_block(&arena, root, block);
    }
    let mut out = String::new();
    format_commonmark(root, &gfm_options(), &mut out).map_err(|_| MediaError::ParseFailure)?;
    Ok(out.into_bytes())
}

/// The deterministic md->html projection (plan acceptance: byte-stable
/// across runs): same parse, comrak's html formatter, GFM tables
/// rendered, raw html suppressed by comrak's safe default.
///
/// # Errors
/// [`MediaError::ParseFailure`] on non-utf8 input;
/// [`MediaError::ExtractTooLarge`] when the input-size pre-check trips.
pub fn render_html(bytes: &[u8], budgets: ExtractBudgets) -> Result<Vec<u8>, MediaError> {
    let source = std::str::from_utf8(bytes).map_err(|_| MediaError::ParseFailure)?;
    if source.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let arena = Arena::new();
    let root = parse_document(&arena, source, &gfm_options());
    let mut out = String::new();
    format_html(root, &gfm_options(), &mut out).map_err(|_| MediaError::ParseFailure)?;
    Ok(out.into_bytes())
}

fn text_node<'a>(arena: &'a Arena<'a>, text: &str) -> &'a AstNode<'a> {
    arena.alloc(AstNode::from(NodeValue::Text(text.to_owned().into())))
}

fn append_block<'a>(arena: &'a Arena<'a>, root: &'a AstNode<'a>, block: &Block) {
    match block {
        Block::Heading { level, text } => {
            let heading = arena.alloc(AstNode::from(NodeValue::Heading(NodeHeading {
                level: *level,
                setext: false,
                closed: false,
            })));
            heading.append(text_node(arena, text));
            root.append(heading);
        }
        Block::Paragraph { text } => {
            let paragraph = arena.alloc(AstNode::from(NodeValue::Paragraph));
            paragraph.append(text_node(arena, text));
            root.append(paragraph);
        }
        Block::CodeFence { language, code } => {
            let mut literal = code.clone();
            if !literal.is_empty() && !literal.ends_with('\n') {
                literal.push('\n');
            }
            let node = arena.alloc(AstNode::from(NodeValue::CodeBlock(Box::new(
                NodeCodeBlock {
                    fenced: true,
                    fence_char: b'`',
                    fence_length: 3,
                    fence_offset: 0,
                    info: language.clone().unwrap_or_default(),
                    literal,
                    closed: true,
                },
            ))));
            root.append(node);
        }
        Block::List { ordered, items } => {
            let list_type = if *ordered {
                comrak::nodes::ListType::Ordered
            } else {
                comrak::nodes::ListType::Bullet
            };
            let list = arena.alloc(AstNode::from(NodeValue::List(NodeList {
                list_type,
                ..NodeList::default()
            })));
            for item in items {
                let entry = arena.alloc(AstNode::from(NodeValue::Item(NodeList::default())));
                let paragraph = arena.alloc(AstNode::from(NodeValue::Paragraph));
                paragraph.append(text_node(arena, item));
                entry.append(paragraph);
                list.append(entry);
            }
            root.append(list);
        }
        Block::Table { header, rows } => {
            let columns = header.len();
            let table = arena.alloc(AstNode::from(NodeValue::Table(Box::new(NodeTable {
                alignments: vec![TableAlignment::None; columns],
                num_columns: columns,
                num_rows: rows.len() + 1,
                num_nonempty_cells: header.len() + rows.iter().map(Vec::len).sum::<usize>(),
            }))));
            append_row(arena, table, header, true);
            for row in rows {
                append_row(arena, table, row, false);
            }
            root.append(table);
        }
    }
}

fn append_row<'a>(arena: &'a Arena<'a>, table: &'a AstNode<'a>, cells: &[String], is_header: bool) {
    let row = arena.alloc(AstNode::from(NodeValue::TableRow(is_header)));
    for cell in cells {
        let node = arena.alloc(AstNode::from(NodeValue::TableCell));
        node.append(text_node(arena, cell));
        row.append(node);
    }
    table.append(row);
}
