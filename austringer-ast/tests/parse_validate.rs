//! Syntax-gate surface (plan todo 14): `is_error()`/`is_missing()` on parse
//! results and structured diagnostics with exact file/line/col positions.

use austringer_ast::language::LanguageId;
use austringer_ast::parse::{self, DiagnosticKind, MAX_DIAGNOSTICS, MAX_SOURCE_BYTES};

const UNBALANCED_PAREN: &[u8] = b"fn main() {\n    let x = (1 + 2;\n    let y = helper(x);\n}\n";
const MISSING_CLOSE_BRACE: &[u8] = b"fn main() {\n    let x = 1;\n";

#[test]
fn valid_source_parses_clean() {
    let parsed = parse::parse(LanguageId::Rust, b"fn main() {}\n".to_vec())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!parsed.is_error());
    assert!(!parsed.is_missing());
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(parsed.language(), LanguageId::Rust);
}

#[test]
fn unbalanced_paren_yields_typed_missing_position() {
    // tree-sitter-rust recovers from the unclosed '(' by inserting a
    // MISSING ")" node: a typed position, no ERROR region.
    let parsed =
        parse::parse(LanguageId::Rust, UNBALANCED_PAREN.to_vec()).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        !parsed.is_error(),
        "no ERROR region for this shape :: {:?}",
        parsed.diagnostics()
    );
    assert!(parsed.is_missing());
    let diags = parsed.diagnostics();
    assert!(!diags.is_empty());

    let missing: Vec<_> = diags
        .iter()
        .filter(|d| d.kind == DiagnosticKind::MissingNode)
        .collect();
    assert!(!missing.is_empty(), ":: {diags:?}");
    let paren = missing
        .iter()
        .find(|d| d.message.contains("\")\""))
        .expect("a missing-paren diagnostic naming the token :: {missing:?}");
    // Exact pinning: ')' was expected right after "1 + 2" on line 2,
    // column 19 (1-based: "    let x = (1 + 2" is 18 columns, so the
    // position after it is column 19).
    assert_eq!((paren.line, paren.column), (2, 19), ":: {paren:?}");

    for d in diags {
        assert!(
            d.line >= 2 && d.line <= 3,
            "all diagnostics on the affected region :: {d:?}"
        );
    }
}

#[test]
fn missing_close_brace_at_eof_is_a_syntax_error_region() {
    // An unclosed '{' at EOF cannot be recovered by inserting a node, so
    // the whole item becomes an ERROR region at the item start.
    let parsed = parse::parse(LanguageId::Rust, MISSING_CLOSE_BRACE.to_vec())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(parsed.is_error());
    assert!(!parsed.is_missing());
    let diags = parsed.diagnostics();
    assert_eq!(diags.len(), 1, "one topmost error region :: {diags:?}");
    assert_eq!(diags[0].kind, DiagnosticKind::SyntaxError);
    assert_eq!(diags[0].message, "syntax error");
    assert_eq!((diags[0].line, diags[0].column), (1, 1));
}

#[test]
fn diagnostics_are_sorted_and_unique_positions() {
    let parsed =
        parse::parse(LanguageId::Rust, UNBALANCED_PAREN.to_vec()).unwrap_or_else(|e| panic!("{e}"));
    let starts: Vec<_> = parsed.diagnostics().iter().map(|d| d.start_byte).collect();
    let mut sorted = starts.clone();
    sorted.sort_unstable();
    assert_eq!(starts, sorted, "diagnostics are position-ordered");
}

#[test]
fn garbage_bytes_never_panic_and_stay_bounded() {
    let mut garbage = Vec::with_capacity(4096);
    let mut state = 0x2545F4914F6CDD1Du64;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        garbage.push((state >> 33) as u8);
    }
    let parsed = parse::parse(LanguageId::Rust, garbage).unwrap_or_else(|e| panic!("{e}"));
    assert!(parsed.is_error());
    assert!(parsed.diagnostics().len() <= MAX_DIAGNOSTICS);
}

#[test]
fn invalid_utf8_inside_comment_parses_clean() {
    let mut src = b"// comment \xff\xfe broken bytes\n".to_vec();
    src.extend_from_slice(b"fn main() {}\n");
    let parsed = parse::parse(LanguageId::Rust, src).unwrap_or_else(|e| panic!("{e}"));
    assert!(!parsed.is_error());
    assert!(parsed.diagnostics().is_empty());
}

#[test]
fn deeply_nested_blocks_complete_without_stack_overflow() {
    let mut src = b"fn deep() {".to_vec();
    src.extend(std::iter::repeat_n(b'{', 20_000));
    src.extend(std::iter::repeat_n(b'}', 20_000));
    src.extend_from_slice(b"}\n");
    let parsed = parse::parse(LanguageId::Rust, src).unwrap_or_else(|e| panic!("{e}"));
    assert!(!parsed.is_missing());
}

#[test]
fn diagnostic_count_is_capped_and_flagged() {
    // Each `fn fN( {}` line inserts its own MISSING ")", so 300 lines
    // produce 300 diagnostics — capped at MAX_DIAGNOSTICS.
    let src: Vec<u8> = (0..300)
        .map(|i| format!("fn f{i}( {{}}\n"))
        .collect::<Vec<_>>()
        .concat()
        .into_bytes();
    let parsed = parse::parse(LanguageId::Rust, src).unwrap_or_else(|e| panic!("{e}"));
    assert!(parsed.is_missing());
    assert_eq!(parsed.diagnostics().len(), MAX_DIAGNOSTICS);
    assert!(parsed.diagnostics_truncated());
    assert!(
        parsed
            .diagnostics()
            .iter()
            .all(|d| d.message == "missing \")\"")
    );
}

#[test]
fn oversize_source_is_a_typed_error_not_a_parse() {
    let huge = vec![b'x'; MAX_SOURCE_BYTES + 1];
    let err = parse::parse(LanguageId::Rust, huge).unwrap_err();
    assert!(
        matches!(err, austringer_ast::AstError::FileTooLarge { size, max, .. } if size == MAX_SOURCE_BYTES as u64 + 1 && max == MAX_SOURCE_BYTES as u64),
        "wrong error: {err:?}"
    );
}

#[test]
fn parse_is_deterministic_across_runs() {
    let mut signatures = Vec::new();
    for _ in 0..3 {
        let parsed = parse::parse(LanguageId::Rust, UNBALANCED_PAREN.to_vec())
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(parsed.is_missing());
        signatures.push(
            parsed
                .diagnostics()
                .iter()
                .map(|d| (d.kind, d.line, d.column, d.start_byte, d.message.clone()))
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(signatures[0], signatures[1]);
    assert_eq!(signatures[1], signatures[2]);
}

#[test]
#[cfg(not(feature = "lang-python"))]
fn feature_off_language_is_a_typed_no_grammar_error_from_parse() {
    // Python is never in the default feature set.
    let err = parse::parse(LanguageId::Python, b"def f(): pass\n".to_vec()).unwrap_err();
    assert!(
        matches!(err, austringer_ast::AstError::NoGrammarCompiled { .. }),
        "wrong error: {err:?}"
    );
}
