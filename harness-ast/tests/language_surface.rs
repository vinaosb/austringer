//! Language surface: detection and feature-set listing (plan todo 14 QA:
//! the full supported-language list is asserted here so it cannot drift
//! from the shipped feature gates).

use harness_ast::language::LanguageId;
use std::path::Path;

#[test]
fn surface_lists_exactly_fifty_languages() {
    assert_eq!(
        LanguageId::ALL.len(),
        50,
        "feature surface must cover 50 languages"
    );
    let mut names: Vec<_> = LanguageId::ALL.iter().map(|l| l.name()).collect();
    let before = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), before, "language names must be unique");
}

#[test]
fn default_feature_set_is_rust_toml_json_bash() {
    let defaults: Vec<&str> = LanguageId::ALL
        .iter()
        .filter(|l| l.is_default_set())
        .map(|l| l.name())
        .collect();
    assert_eq!(defaults, ["bash", "json", "rust", "toml"]);
}

#[test]
fn every_language_names_a_feature_gate() {
    for lang in LanguageId::ALL {
        let gate = lang.feature_gate();
        assert!(gate.starts_with("lang-"), "{gate} for {}", lang.name());
        assert_eq!(gate.trim_start_matches("lang-"), lang.name());
    }
}

#[test]
fn extension_detection_maps_common_file_types() {
    let cases = [
        ("rs", LanguageId::Rust),
        ("toml", LanguageId::Toml),
        ("json", LanguageId::Json),
        ("sh", LanguageId::Bash),
        ("py", LanguageId::Python),
        ("pyi", LanguageId::Python),
        ("js", LanguageId::JavaScript),
        ("mjs", LanguageId::JavaScript),
        ("ts", LanguageId::TypeScript),
        ("tsx", LanguageId::Tsx),
        ("go", LanguageId::Go),
        ("c", LanguageId::C),
        ("h", LanguageId::C),
        ("cpp", LanguageId::Cpp),
        ("cs", LanguageId::CSharp),
        ("java", LanguageId::Java),
        ("swift", LanguageId::Swift),
        ("scala", LanguageId::Scala),
        ("rb", LanguageId::Ruby),
        ("php", LanguageId::Php),
        ("lua", LanguageId::Lua),
        ("jl", LanguageId::Julia),
        ("hs", LanguageId::Haskell),
        ("ml", LanguageId::Ocaml),
        ("mli", LanguageId::OcamlInterface),
        ("ex", LanguageId::Elixir),
        ("erl", LanguageId::Erlang),
        ("yaml", LanguageId::Yaml),
        ("yml", LanguageId::Yaml),
        ("html", LanguageId::Html),
        ("css", LanguageId::Css),
        ("zig", LanguageId::Zig),
        ("nix", LanguageId::Nix),
        ("cmake", LanguageId::Cmake),
        ("tf", LanguageId::Hcl),
        ("dart", LanguageId::Dart),
        ("elm", LanguageId::Elm),
        ("rst", LanguageId::Rst),
        ("sol", LanguageId::Solidity),
        ("scm", LanguageId::Scheme),
        ("el", LanguageId::Elisp),
        ("ps1", LanguageId::Powershell),
        ("gradle", LanguageId::Groovy),
        ("nim", LanguageId::Nim),
        ("mm", LanguageId::ObjC),
        ("v", LanguageId::Verilog),
        ("fs", LanguageId::FSharp),
        ("f90", LanguageId::Fortran),
        ("m", LanguageId::Matlab),
        ("graphql", LanguageId::Graphql),
    ];
    for (ext, want) in cases {
        assert_eq!(
            LanguageId::from_extension(ext),
            Some(want),
            "extension .{ext} must detect {}",
            want.name()
        );
    }
}

#[test]
fn extension_detection_is_case_insensitive_where_unambiguous() {
    assert_eq!(LanguageId::from_extension("R"), Some(LanguageId::R));
    assert_eq!(LanguageId::from_extension("Rs"), Some(LanguageId::Rust));
    assert_eq!(
        LanguageId::from_extension("PS1"),
        Some(LanguageId::Powershell)
    );
}

#[test]
fn unknown_extensions_and_extensions_without_grammars_degrade_to_none() {
    assert_eq!(LanguageId::from_extension("xyzzy"), None);
    assert_eq!(
        LanguageId::from_extension("txt"),
        None,
        "plain text has no grammar"
    );
    assert_eq!(LanguageId::from_extension(""), None);
    assert_eq!(LanguageId::from_extension("exe"), None);
}

#[test]
fn path_detection_covers_extensionless_makefiles() {
    assert_eq!(
        LanguageId::from_path(Path::new("Makefile")),
        Some(LanguageId::Make)
    );
    assert_eq!(
        LanguageId::from_path(Path::new("makefile")),
        Some(LanguageId::Make)
    );
    assert_eq!(
        LanguageId::from_path(Path::new("GNUmakefile")),
        Some(LanguageId::Make)
    );
    assert_eq!(
        LanguageId::from_path(Path::new("src/GNUmakefile")),
        Some(LanguageId::Make)
    );
    assert_eq!(
        LanguageId::from_path(Path::new("src/build.mk")),
        Some(LanguageId::Make)
    );
    assert_eq!(LanguageId::from_path(Path::new("README")), None);
    assert_eq!(
        LanguageId::from_path(Path::new("a/b/c.py")),
        Some(LanguageId::Python)
    );
}

#[test]
fn names_round_trip() {
    for lang in LanguageId::ALL {
        assert_eq!(LanguageId::from_name(lang.name()), Some(*lang));
    }
    assert_eq!(LanguageId::from_name("not-a-language"), None);
}
