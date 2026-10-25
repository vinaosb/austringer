//! Lexical definition-line extractor feeding the BM25 `defs` boost field.
//!
//! This is deliberately a cheap token-shaped heuristic — real symbol tables
//! arrive with `austringer-ast` (plan todo 14). It recognizes the common
//! definition keyword set across Rust/Python/Go/TS-family sources so that
//! "search `foo` ranks `fn foo` above `foo()` call sites" works day one.

const DEF_KEYWORDS: &[&str] = &[
    "fn",
    "struct",
    "enum",
    "union",
    "trait",
    "impl",
    "type",
    "mod",
    "const",
    "static",
    "macro_rules",
    "class",
    "interface",
    "def",
    "func",
    "function",
    "module",
];

const MODIFIERS: &[&str] = &[
    "pub(crate)",
    "pub(super)",
    "pub",
    "async",
    "unsafe",
    "extern",
];

fn is_symbol_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_symbol_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// Leading definition symbol of one line, if the line is definition-shaped.
fn line_symbol(line: &str) -> Option<String> {
    let rest = skip_modifiers(line.trim_start());
    let keyword_len = rest
        .chars()
        .take_while(|c| is_symbol_char(*c))
        .map(char::len_utf8)
        .sum();
    if !DEF_KEYWORDS.contains(&&rest[..keyword_len]) {
        return None;
    }
    let after = rest[keyword_len..]
        .trim_start_matches([' ', '\t'])
        .trim_start_matches('!')
        .trim_start_matches([' ', '\t']);
    let symbol: String = after.chars().take_while(|c| is_symbol_char(*c)).collect();
    symbol
        .starts_with(|c: char| is_symbol_start(c))
        .then_some(symbol)
}

/// Strips leading modifier tokens (`pub`, `async`, `unsafe`, `extern "ABI"`).
fn skip_modifiers(mut rest: &str) -> &str {
    loop {
        let first_len = rest.split([' ', '\t']).next().map_or(0, str::len);
        if !MODIFIERS.contains(&&rest[..first_len]) {
            return rest;
        }
        rest = rest[first_len..].trim_start();
        // `extern "C" fn` carries a linkage string after the modifier.
        if let Some(tail) = rest.strip_prefix('"')
            && let Some(end) = tail.find('"')
        {
            rest = tail[end + 1..].trim_start();
        }
    }
}

/// Space-joined definition symbols found on definition-shaped lines.
pub(crate) fn definition_symbols(content: &str) -> String {
    let mut out = String::new();
    for line in content.lines() {
        if let Some(symbol) = line_symbol(line) {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&symbol);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::definition_symbols;

    #[test]
    fn extracts_rust_python_and_typescript_definitions() {
        let content = "\
pub fn alpha() {}
fn beta(x: u32) {}
    async unsafe fn gamma() {}
pub struct Delta;
enum Epsilon {}
impl Zeta {}
const WIDGET_COUNT: u32 = 3;
class Eta:
    def theta(self):
function iota() {}
let not_a_def = call();
";
        assert_eq!(
            definition_symbols(content),
            "alpha beta gamma Delta Epsilon Zeta WIDGET_COUNT Eta theta iota"
        );
    }

    #[test]
    fn no_definitions_yields_empty_string() {
        assert_eq!(definition_symbols("let x = alpha_beta();\ncall();\n"), "");
    }
}
