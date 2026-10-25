//! Tag extraction (plan todo 14): definitions with kind+span and
//! references per file, asserted on CONTENT (names, kinds, exact spans),
//! never on counts alone. Expected byte offsets are derived from the
//! fixture by independent byte search, not hand-counted.

use austringer_ast::language::LanguageId;
use austringer_ast::parse;
use austringer_ast::tags::{self, SymbolKind};

const RUST_FIXTURE: &[u8] = b"//! doc\nuse std::collections::HashMap;\n\nmod inner {\n    pub struct Point {\n        pub x: i32,\n    }\n\n    impl Point {\n        pub fn origin() -> Point {\n            Point { x: 0 }\n        }\n    }\n}\n\nfn main() {\n    let p = inner::Point::origin();\n    println!(\"{:?}\", p);\n}\n\nconst LIMIT: usize = 10;\ntype Alias = HashMap<String, i32>;\n";

fn first_occurrence(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len())
        .position(|w| w == needle)
        .unwrap_or_else(|| {
            panic!(
                "needle {:?} not in fixture",
                String::from_utf8_lossy(needle)
            )
        })
}

fn analyze(src: &[u8]) -> (parse::ParseResult, tags::TagTable) {
    let parsed = parse::parse(LanguageId::Rust, src.to_vec()).unwrap_or_else(|e| panic!("{e}"));
    let table = tags::extract(&parsed).unwrap_or_else(|e| panic!("{e}"));
    (parsed, table)
}

#[test]
fn rust_definitions_carry_names_kinds_and_exact_spans() {
    let (parsed, table) = analyze(RUST_FIXTURE);

    let point_start = first_occurrence(RUST_FIXTURE, b"pub struct Point") + "pub struct ".len();
    let origin_start = first_occurrence(RUST_FIXTURE, b"origin");
    let main_start = first_occurrence(RUST_FIXTURE, b"fn main()") + "fn ".len();
    let limit_start = first_occurrence(RUST_FIXTURE, b"const LIMIT") + "const ".len();
    let alias_start = first_occurrence(RUST_FIXTURE, b"type Alias") + "type ".len();

    let by_name = |name: &str| {
        table
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("no def named {name} :: {:?}", table.definitions))
    };

    assert!(
        table
            .definitions
            .iter()
            .any(|d| d.name == "inner" && d.kind == SymbolKind::Module)
    );
    assert_eq!(by_name("Point").name_span.start_byte, point_start);
    assert_eq!(by_name("origin").name_span.start_byte, origin_start);
    assert_eq!(by_name("main").name_span.start_byte, main_start);
    assert_eq!(by_name("LIMIT").name_span.start_byte, limit_start);
    assert_eq!(by_name("Alias").name_span.start_byte, alias_start);
    assert_eq!(by_name("origin").kind, SymbolKind::Function);
    assert_eq!(by_name("Point").kind, SymbolKind::Struct);
    assert_eq!(by_name("LIMIT").kind, SymbolKind::Constant);
    assert_eq!(by_name("Alias").kind, SymbolKind::Type);

    for d in &table.definitions {
        assert_eq!(
            &parsed.source()[d.name_span.start_byte..d.name_span.end_byte],
            d.name.as_bytes(),
            "span must slice to the definition name :: {}",
            d.name
        );
    }

    let origin = by_name("origin");
    let container = origin.def_span.expect("origin carries a container span");
    assert!(container.start_byte <= origin.name_span.start_byte);
    assert!(
        container.end_byte > origin.name_span.end_byte,
        "container spans the whole fn item"
    );
}

#[test]
fn rust_references_capture_calls_and_macro_invocations() {
    let (parsed, table) = analyze(RUST_FIXTURE);
    let ref_names: Vec<(&str, &SymbolKind)> = table
        .references
        .iter()
        .map(|r| (r.name.as_str(), &r.kind))
        .collect();
    assert!(
        ref_names.contains(&("origin", &SymbolKind::Call)),
        "call origin :: {ref_names:?}"
    );
    assert!(
        ref_names.contains(&("println", &SymbolKind::Call)),
        "macro println! :: {ref_names:?}"
    );
    assert!(
        !table.references.iter().any(|r| r.name == "Point"),
        "type usage is not a call-ref"
    );
    for r in &table.references {
        assert_eq!(
            &parsed.source()[r.span.start_byte..r.span.end_byte],
            r.name.as_bytes(),
            "reference span slices to the name :: {}",
            r.name
        );
    }
}

#[test]
fn definitions_and_references_are_position_ordered() {
    let (_, table) = analyze(RUST_FIXTURE);
    let dstarts: Vec<usize> = table
        .definitions
        .iter()
        .map(|d| d.name_span.start_byte)
        .collect();
    let mut ds = dstarts.clone();
    ds.sort_unstable();
    assert_eq!(dstarts, ds);
    let rstarts: Vec<usize> = table.references.iter().map(|r| r.span.start_byte).collect();
    let mut rs = rstarts.clone();
    rs.sort_unstable();
    assert_eq!(rstarts, rs);
}

#[test]
fn toml_tables_and_pairs_are_definitions() {
    let src =
        b"[package]\nname = \"austringer-ast\"\n\n[dependencies]\nserde = { version = \"1\" }\n";
    let parsed = parse::parse(LanguageId::Toml, src.to_vec()).unwrap_or_else(|e| panic!("{e}"));
    let table = tags::extract(&parsed).unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<(&str, &SymbolKind)> = table
        .definitions
        .iter()
        .map(|d| (d.name.as_str(), &d.kind))
        .collect();
    assert!(
        names.contains(&("package", &SymbolKind::Section)),
        ":: {names:?}"
    );
    assert!(
        names.contains(&("dependencies", &SymbolKind::Section)),
        ":: {names:?}"
    );
    assert!(
        names.contains(&("name", &SymbolKind::Variable)),
        ":: {names:?}"
    );
    assert!(
        names.contains(&("serde", &SymbolKind::Variable)),
        ":: {names:?}"
    );
    assert!(table.references.is_empty(), "toml has no references");
}

#[test]
fn json_property_keys_are_definitions() {
    let src = b"{ \"name\": \"austringer\", \"nested\": { \"inner\": 1 } }";
    let parsed = parse::parse(LanguageId::Json, src.to_vec()).unwrap_or_else(|e| panic!("{e}"));
    let table = tags::extract(&parsed).unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<&str> = table.definitions.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"name"), ":: {names:?}");
    assert!(names.contains(&"inner"), ":: {names:?}");
    assert!(table.references.is_empty(), "json has no references");
}

#[test]
fn bash_functions_variables_and_calls() {
    let src = b"#!/usr/bin/env bash\nGREETING=\"hi\"\nfunction deploy {\n  echo \"$GREETING\"\n}\ndeploy\n";
    let parsed = parse::parse(LanguageId::Bash, src.to_vec()).unwrap_or_else(|e| panic!("{e}"));
    let table = tags::extract(&parsed).unwrap_or_else(|e| panic!("{e}"));
    let def_names: Vec<(&str, &SymbolKind)> = table
        .definitions
        .iter()
        .map(|d| (d.name.as_str(), &d.kind))
        .collect();
    assert!(
        def_names.contains(&("deploy", &SymbolKind::Function)),
        ":: {def_names:?}"
    );
    assert!(
        def_names.contains(&("GREETING", &SymbolKind::Variable)),
        ":: {def_names:?}"
    );
    let refs: Vec<&str> = table.references.iter().map(|r| r.name.as_str()).collect();
    assert!(refs.contains(&"deploy"), "call site captured :: {refs:?}");
    assert!(refs.contains(&"echo"), "command call captured :: {refs:?}");
}

#[test]
fn empty_source_yields_empty_tables() {
    for (lang, src) in [
        (LanguageId::Rust, &b""[..]),
        (LanguageId::Json, &b"{}"[..]),
        (LanguageId::Toml, &b""[..]),
        (LanguageId::Bash, &b""[..]),
    ] {
        let parsed = parse::parse(lang, src.to_vec()).unwrap_or_else(|e| panic!("{e}"));
        let table = tags::extract(&parsed).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            table.definitions.is_empty(),
            "{lang:?} :: {:?}",
            table.definitions
        );
        assert!(
            table.references.is_empty(),
            "{lang:?} :: {:?}",
            table.references
        );
    }
}

#[test]
fn broken_source_still_extracts_symbols() {
    // Tags from the parseable prefix; the pipeline does not stop on errors.
    let (_, table) = analyze(b"fn good() {}\nfn broken() { let x = (1 + 2;\n");
    let names: Vec<&str> = table.definitions.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"good"), ":: {names:?}");
}

#[test]
fn symbols_survive_invalid_utf8_without_panicking() {
    // \xff splits what looks like an identifier; the grammar recovers and
    // captures the first fragment. The contract: extraction completes and
    // produces valid UTF-8 names (lossy), never a panic.
    let (_, table) = analyze(b"fn good() {}\nfn b\xffd() {}\n");
    let names: Vec<&str> = table.definitions.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"good"), ":: {names:?}");
    assert!(
        names.iter().any(|n| n.starts_with('b')),
        "fragment name captured :: {names:?}"
    );
}
