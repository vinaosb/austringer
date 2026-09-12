//! Grammar registry: language id → tree-sitter language + embedded tags
//! query. One `#[cfg]` arm per language feature; languages whose feature
//! is off resolve to `None` and callers degrade to text tools.

use crate::language::LanguageId;

/// A compiled-in grammar plus its tags query source.
pub(crate) struct Grammar {
    /// The tree-sitter language handle (`LanguageFn` converted once).
    pub language: tree_sitter::Language,
    /// Embedded `tags.scm`-style query (definitions + references).
    pub tags_query: &'static str,
}

/// Resolve the grammar for `language`, or `None` when its feature gate is
/// not compiled in.
fn grammar(
    language: impl Into<tree_sitter::Language>,
    tags_query: &'static str,
) -> (tree_sitter::Language, &'static str) {
    (language.into(), tags_query)
}
pub(crate) fn grammar_for(language: LanguageId) -> Option<Grammar> {
    let (language, tags_query) = match language {
        #[cfg(feature = "lang-rust")]
        LanguageId::Rust => grammar(tree_sitter_rust::LANGUAGE, include_str!("queries/rust.scm")),
        #[cfg(feature = "lang-toml")]
        LanguageId::Toml => grammar(
            tree_sitter_toml_ng::LANGUAGE,
            include_str!("queries/toml.scm"),
        ),
        #[cfg(feature = "lang-json")]
        LanguageId::Json => grammar(tree_sitter_json::LANGUAGE, include_str!("queries/json.scm")),
        #[cfg(feature = "lang-bash")]
        LanguageId::Bash => grammar(tree_sitter_bash::LANGUAGE, include_str!("queries/bash.scm")),
        #[cfg(feature = "lang-python")]
        LanguageId::Python => grammar(
            tree_sitter_python::LANGUAGE,
            include_str!("queries/python.scm"),
        ),
        #[cfg(feature = "lang-javascript")]
        LanguageId::JavaScript => grammar(
            tree_sitter_javascript::LANGUAGE,
            include_str!("queries/javascript.scm"),
        ),
        #[cfg(feature = "lang-typescript")]
        LanguageId::TypeScript => grammar(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            include_str!("queries/typescript.scm"),
        ),
        #[cfg(feature = "lang-tsx")]
        LanguageId::Tsx => grammar(
            tree_sitter_typescript::LANGUAGE_TSX,
            include_str!("queries/tsx.scm"),
        ),
        #[cfg(feature = "lang-go")]
        LanguageId::Go => grammar(tree_sitter_go::LANGUAGE, include_str!("queries/go.scm")),
        #[cfg(feature = "lang-c")]
        LanguageId::C => grammar(tree_sitter_c::LANGUAGE, include_str!("queries/c.scm")),
        #[cfg(feature = "lang-cpp")]
        LanguageId::Cpp => grammar(tree_sitter_cpp::LANGUAGE, include_str!("queries/cpp.scm")),
        #[cfg(feature = "lang-c-sharp")]
        LanguageId::CSharp => grammar(
            tree_sitter_c_sharp::LANGUAGE,
            include_str!("queries/c-sharp.scm"),
        ),
        #[cfg(feature = "lang-java")]
        LanguageId::Java => grammar(tree_sitter_java::LANGUAGE, include_str!("queries/java.scm")),
        #[cfg(feature = "lang-swift")]
        LanguageId::Swift => grammar(
            tree_sitter_swift::LANGUAGE,
            include_str!("queries/swift.scm"),
        ),
        #[cfg(feature = "lang-scala")]
        LanguageId::Scala => grammar(
            tree_sitter_scala::LANGUAGE,
            include_str!("queries/scala.scm"),
        ),
        #[cfg(feature = "lang-ruby")]
        LanguageId::Ruby => grammar(tree_sitter_ruby::LANGUAGE, include_str!("queries/ruby.scm")),
        #[cfg(feature = "lang-php")]
        LanguageId::Php => grammar(
            tree_sitter_php::LANGUAGE_PHP,
            include_str!("queries/php.scm"),
        ),
        #[cfg(feature = "lang-lua")]
        LanguageId::Lua => grammar(tree_sitter_lua::LANGUAGE, include_str!("queries/lua.scm")),
        #[cfg(feature = "lang-r")]
        LanguageId::R => grammar(tree_sitter_r::LANGUAGE, include_str!("queries/r.scm")),
        #[cfg(feature = "lang-julia")]
        LanguageId::Julia => grammar(
            tree_sitter_julia::LANGUAGE,
            include_str!("queries/julia.scm"),
        ),
        #[cfg(feature = "lang-haskell")]
        LanguageId::Haskell => grammar(
            tree_sitter_haskell::LANGUAGE,
            include_str!("queries/haskell.scm"),
        ),
        #[cfg(feature = "lang-ocaml")]
        LanguageId::Ocaml => grammar(
            tree_sitter_ocaml::LANGUAGE_OCAML,
            include_str!("queries/ocaml.scm"),
        ),
        #[cfg(feature = "lang-ocaml-interface")]
        LanguageId::OcamlInterface => grammar(
            tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE,
            include_str!("queries/ocaml-interface.scm"),
        ),
        #[cfg(feature = "lang-elixir")]
        LanguageId::Elixir => grammar(
            tree_sitter_elixir::LANGUAGE,
            include_str!("queries/elixir.scm"),
        ),
        #[cfg(feature = "lang-erlang")]
        LanguageId::Erlang => grammar(
            tree_sitter_erlang::LANGUAGE,
            include_str!("queries/erlang.scm"),
        ),
        #[cfg(feature = "lang-yaml")]
        LanguageId::Yaml => grammar(tree_sitter_yaml::LANGUAGE, include_str!("queries/yaml.scm")),
        #[cfg(feature = "lang-html")]
        LanguageId::Html => grammar(tree_sitter_html::LANGUAGE, include_str!("queries/html.scm")),
        #[cfg(feature = "lang-css")]
        LanguageId::Css => grammar(tree_sitter_css::LANGUAGE, include_str!("queries/css.scm")),
        #[cfg(feature = "lang-graphql")]
        LanguageId::Graphql => grammar(
            tree_sitter_graphql::LANGUAGE,
            include_str!("queries/graphql.scm"),
        ),
        #[cfg(feature = "lang-zig")]
        LanguageId::Zig => grammar(tree_sitter_zig::LANGUAGE, include_str!("queries/zig.scm")),
        #[cfg(feature = "lang-nix")]
        LanguageId::Nix => grammar(tree_sitter_nix::LANGUAGE, include_str!("queries/nix.scm")),
        #[cfg(feature = "lang-make")]
        LanguageId::Make => grammar(tree_sitter_make::LANGUAGE, include_str!("queries/make.scm")),
        #[cfg(feature = "lang-cmake")]
        LanguageId::Cmake => grammar(
            tree_sitter_cmake::LANGUAGE,
            include_str!("queries/cmake.scm"),
        ),
        #[cfg(feature = "lang-hcl")]
        LanguageId::Hcl => grammar(tree_sitter_hcl::LANGUAGE, include_str!("queries/hcl.scm")),
        #[cfg(feature = "lang-regex")]
        LanguageId::Regex => grammar(
            tree_sitter_regex::LANGUAGE,
            include_str!("queries/regex.scm"),
        ),
        #[cfg(feature = "lang-dart")]
        LanguageId::Dart => grammar(tree_sitter_dart::LANGUAGE, include_str!("queries/dart.scm")),
        #[cfg(feature = "lang-elm")]
        LanguageId::Elm => grammar(tree_sitter_elm::LANGUAGE, include_str!("queries/elm.scm")),
        #[cfg(feature = "lang-rst")]
        LanguageId::Rst => grammar(tree_sitter_rst::LANGUAGE, include_str!("queries/rst.scm")),
        #[cfg(feature = "lang-solidity")]
        LanguageId::Solidity => grammar(
            tree_sitter_solidity::LANGUAGE,
            include_str!("queries/solidity.scm"),
        ),
        #[cfg(feature = "lang-scheme")]
        LanguageId::Scheme => grammar(
            tree_sitter_scheme::LANGUAGE,
            include_str!("queries/scheme.scm"),
        ),
        #[cfg(feature = "lang-commonlisp")]
        LanguageId::CommonLisp => grammar(
            tree_sitter_commonlisp::LANGUAGE_COMMONLISP,
            include_str!("queries/commonlisp.scm"),
        ),
        #[cfg(feature = "lang-elisp")]
        LanguageId::Elisp => grammar(
            tree_sitter_elisp::LANGUAGE,
            include_str!("queries/elisp.scm"),
        ),
        #[cfg(feature = "lang-powershell")]
        LanguageId::Powershell => grammar(
            tree_sitter_powershell::LANGUAGE,
            include_str!("queries/powershell.scm"),
        ),
        #[cfg(feature = "lang-groovy")]
        LanguageId::Groovy => grammar(
            tree_sitter_groovy::LANGUAGE,
            include_str!("queries/groovy.scm"),
        ),
        #[cfg(feature = "lang-nim")]
        LanguageId::Nim => grammar(tree_sitter_nim::LANGUAGE, include_str!("queries/nim.scm")),
        #[cfg(feature = "lang-objc")]
        LanguageId::ObjC => grammar(tree_sitter_objc::LANGUAGE, include_str!("queries/objc.scm")),
        #[cfg(feature = "lang-verilog")]
        LanguageId::Verilog => grammar(
            tree_sitter_verilog::LANGUAGE,
            include_str!("queries/verilog.scm"),
        ),
        #[cfg(feature = "lang-fsharp")]
        LanguageId::FSharp => grammar(
            tree_sitter_fsharp::LANGUAGE_FSHARP,
            include_str!("queries/fsharp.scm"),
        ),
        #[cfg(feature = "lang-fortran")]
        LanguageId::Fortran => grammar(
            tree_sitter_fortran::LANGUAGE,
            include_str!("queries/fortran.scm"),
        ),
        #[cfg(feature = "lang-matlab")]
        LanguageId::Matlab => grammar(
            tree_sitter_matlab::LANGUAGE,
            include_str!("queries/matlab.scm"),
        ),
        // Reachable whenever any lang feature is off; with ALL features
        // enabled rustc proves it unreachable. Feature-matrix wildcard,
        // not a defect.
        #[allow(unreachable_patterns)]
        _ => return None,
    };
    Some(Grammar {
        language,
        tags_query,
    })
}
