//! Language identification: the closed set of languages this crate supports
//! across its feature surface, plus file-extension detection.
//!
//! Extension mapping is necessarily opinionated where ecosystems collide:
//! `.h` → C (C++ headers parse under C too), `.m` → Matlab (ObjC keeps
//! `.mm`), `.v`/`.sv` → Verilog, `.cl`/`.lisp` → CommonLisp, `.el` → Elisp,
//! `.scm`/`.ss` → Scheme. `Regex` has no file extension — it is reachable
//! only via an explicit [`LanguageId::Regex`].

use std::path::Path;

/// One supported tree-sitter language. Grammar crates are feature-gated
/// per language (`lang-<name>` in `Cargo.toml`); detection never depends on
/// which features are compiled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LanguageId {
    // Default-feature set (compiled by `default`).
    Bash,
    Json,
    Rust,
    Toml,
    // Opt-in set (compiled by `langs-extra` or individual `lang-*` features).
    C,
    Cmake,
    CommonLisp,
    Cpp,
    CSharp,
    Css,
    Dart,
    Elixir,
    Elisp,
    Elm,
    Erlang,
    Fortran,
    FSharp,
    Go,
    Graphql,
    Groovy,
    Haskell,
    Hcl,
    Html,
    Java,
    JavaScript,
    Julia,
    Lua,
    Make,
    Matlab,
    Nix,
    Nim,
    ObjC,
    Ocaml,
    OcamlInterface,
    Php,
    Powershell,
    Python,
    R,
    Regex,
    Rst,
    Ruby,
    Scala,
    Scheme,
    Solidity,
    Swift,
    TypeScript,
    Tsx,
    Verilog,
    Yaml,
    Zig,
}

impl LanguageId {
    /// The full feature surface, default set first (alphabetically), then
    /// the opt-in set (alphabetically). 50 languages in total.
    pub const ALL: &'static [LanguageId] = &[
        LanguageId::Bash,
        LanguageId::Json,
        LanguageId::Rust,
        LanguageId::Toml,
        LanguageId::C,
        LanguageId::Cmake,
        LanguageId::CommonLisp,
        LanguageId::Cpp,
        LanguageId::CSharp,
        LanguageId::Css,
        LanguageId::Dart,
        LanguageId::Elixir,
        LanguageId::Elisp,
        LanguageId::Elm,
        LanguageId::Erlang,
        LanguageId::Fortran,
        LanguageId::FSharp,
        LanguageId::Go,
        LanguageId::Graphql,
        LanguageId::Groovy,
        LanguageId::Haskell,
        LanguageId::Hcl,
        LanguageId::Html,
        LanguageId::Java,
        LanguageId::JavaScript,
        LanguageId::Julia,
        LanguageId::Lua,
        LanguageId::Make,
        LanguageId::Matlab,
        LanguageId::Nix,
        LanguageId::Nim,
        LanguageId::ObjC,
        LanguageId::Ocaml,
        LanguageId::OcamlInterface,
        LanguageId::Php,
        LanguageId::Powershell,
        LanguageId::Python,
        LanguageId::R,
        LanguageId::Regex,
        LanguageId::Rst,
        LanguageId::Ruby,
        LanguageId::Scala,
        LanguageId::Scheme,
        LanguageId::Solidity,
        LanguageId::Swift,
        LanguageId::TypeScript,
        LanguageId::Tsx,
        LanguageId::Verilog,
        LanguageId::Yaml,
        LanguageId::Zig,
    ];

    /// Stable identifier, also the cargo feature name suffix (`lang-<name>`).
    pub const fn name(self) -> &'static str {
        match self {
            LanguageId::Bash => "bash",
            LanguageId::Json => "json",
            LanguageId::Rust => "rust",
            LanguageId::Toml => "toml",
            LanguageId::C => "c",
            LanguageId::Cmake => "cmake",
            LanguageId::CommonLisp => "commonlisp",
            LanguageId::Cpp => "cpp",
            LanguageId::CSharp => "c-sharp",
            LanguageId::Css => "css",
            LanguageId::Dart => "dart",
            LanguageId::Elixir => "elixir",
            LanguageId::Elisp => "elisp",
            LanguageId::Elm => "elm",
            LanguageId::Erlang => "erlang",
            LanguageId::Fortran => "fortran",
            LanguageId::FSharp => "fsharp",
            LanguageId::Go => "go",
            LanguageId::Graphql => "graphql",
            LanguageId::Groovy => "groovy",
            LanguageId::Haskell => "haskell",
            LanguageId::Hcl => "hcl",
            LanguageId::Html => "html",
            LanguageId::Java => "java",
            LanguageId::JavaScript => "javascript",
            LanguageId::Julia => "julia",
            LanguageId::Lua => "lua",
            LanguageId::Make => "make",
            LanguageId::Matlab => "matlab",
            LanguageId::Nix => "nix",
            LanguageId::Nim => "nim",
            LanguageId::ObjC => "objc",
            LanguageId::Ocaml => "ocaml",
            LanguageId::OcamlInterface => "ocaml-interface",
            LanguageId::Php => "php",
            LanguageId::Powershell => "powershell",
            LanguageId::Python => "python",
            LanguageId::R => "r",
            LanguageId::Regex => "regex",
            LanguageId::Rst => "rst",
            LanguageId::Ruby => "ruby",
            LanguageId::Scala => "scala",
            LanguageId::Scheme => "scheme",
            LanguageId::Solidity => "solidity",
            LanguageId::Swift => "swift",
            LanguageId::TypeScript => "typescript",
            LanguageId::Tsx => "tsx",
            LanguageId::Verilog => "verilog",
            LanguageId::Yaml => "yaml",
            LanguageId::Zig => "zig",
        }
    }

    /// Whether this language belongs to the default feature set.
    pub const fn is_default_set(self) -> bool {
        matches!(
            self,
            LanguageId::Bash | LanguageId::Json | LanguageId::Rust | LanguageId::Toml
        )
    }

    /// Cargo feature gate that compiles this language's grammar:
    /// `lang-<name>`. The `langs-extra` feature aggregates all non-default
    /// gates.
    pub fn feature_gate(self) -> String {
        format!("lang-{}", self.name())
    }

    /// Inverse of [`LanguageId::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        LanguageId::ALL
            .iter()
            .copied()
            .find(|lang| lang.name() == name)
    }

    /// Detect a language from a file extension (without the dot). ASCII
    /// case-insensitive. Returns `None` for unknown extensions — callers
    /// degrade to text tools (never an error).
    pub fn from_extension(ext: &str) -> Option<Self> {
        let lower = ext.to_ascii_lowercase();
        Some(match lower.as_str() {
            "rs" => LanguageId::Rust,
            "toml" => LanguageId::Toml,
            "json" => LanguageId::Json,
            "sh" | "bash" => LanguageId::Bash,
            "py" | "pyi" => LanguageId::Python,
            "js" | "mjs" | "cjs" | "jsx" => LanguageId::JavaScript,
            "ts" | "mts" | "cts" => LanguageId::TypeScript,
            "tsx" => LanguageId::Tsx,
            "go" => LanguageId::Go,
            "c" | "h" => LanguageId::C,
            "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" => LanguageId::Cpp,
            "cs" => LanguageId::CSharp,
            "java" => LanguageId::Java,
            "swift" => LanguageId::Swift,
            "scala" | "sc" => LanguageId::Scala,
            "rb" => LanguageId::Ruby,
            "php" => LanguageId::Php,
            "lua" => LanguageId::Lua,
            "r" => LanguageId::R,
            "jl" => LanguageId::Julia,
            "hs" => LanguageId::Haskell,
            "ml" => LanguageId::Ocaml,
            "mli" => LanguageId::OcamlInterface,
            "ex" | "exs" => LanguageId::Elixir,
            "erl" | "hrl" => LanguageId::Erlang,
            "yaml" | "yml" => LanguageId::Yaml,
            "html" | "htm" => LanguageId::Html,
            "css" => LanguageId::Css,
            "graphql" | "gql" => LanguageId::Graphql,
            "zig" => LanguageId::Zig,
            "nix" => LanguageId::Nix,
            "mk" | "mak" => LanguageId::Make,
            "cmake" => LanguageId::Cmake,
            "hcl" | "tf" | "tfvars" => LanguageId::Hcl,
            "dart" => LanguageId::Dart,
            "elm" => LanguageId::Elm,
            "rst" => LanguageId::Rst,
            "sol" => LanguageId::Solidity,
            "scm" | "ss" => LanguageId::Scheme,
            "lisp" | "cl" => LanguageId::CommonLisp,
            "el" => LanguageId::Elisp,
            "ps1" | "psm1" | "psd1" => LanguageId::Powershell,
            "groovy" | "gradle" => LanguageId::Groovy,
            "nim" => LanguageId::Nim,
            "m" => LanguageId::Matlab,
            "mm" => LanguageId::ObjC,
            "v" | "sv" => LanguageId::Verilog,
            "fs" | "fsi" | "fsx" => LanguageId::FSharp,
            "f" | "for" | "f90" | "f95" | "f03" | "fpp" => LanguageId::Fortran,
            _ => return None,
        })
    }

    /// Detect a language from a path: extensionless makefile names first,
    /// then the extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let file_name = path.file_name()?.to_str()?;
        match file_name.to_ascii_lowercase().as_str() {
            "makefile" | "gnumakefile" => return Some(LanguageId::Make),
            _ => {}
        }
        Self::from_extension(path.extension()?.to_str()?)
    }
}
