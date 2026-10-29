//! Tree-sitter tag tables and syntax validation gates (plan todo 14).
//!
//! One engine, three surfaces:
//! - [`parse`] — byte-oriented parse with `is_error()` / `is_missing()` and
//!   position-exact, capped [`Diagnostic`]s (file, line, col).
//! - [`tags`] — `tags.scm`-style definition/reference extraction into a
//!   serializable symbol table (name, kind, spans).
//! - [`analyze::Analyzer`] — the orchestrator: language detection, a
//!   Blake3+mtime-keyed persistent table cache under a caller-chosen dir,
//!   and the **no-grammar degradation contract**: unsupported or
//!   not-compiled-in languages return [`analyze::AnalyzeOutcome::NoGrammar`]
//!   — never an error — so pipelines fall back to text tools.
//!
//! # Feature surface (50 languages)
//!
//! Default feature (4 grammars, keeps cold builds to four C compilations —
//! the file types a Rust agent austringer touches most):
//! **bash, json, rust, toml**.
//!
//! Opt-in `langs-extra` (46 more; also individually via `lang-<name>`):
//! **c, cmake, commonlisp, cpp, c-sharp, css, dart, elixir, elisp, elm,
//! erlang, fortran, fsharp, go, graphql, groovy, haskell, hcl, html, java,
//! javascript, julia, lua, make, matlab, nim, objc, ocaml, ocaml-interface,
//! php, powershell, python, r, regex, rst, ruby, scala, scheme, solidity,
//! swift, typescript, tsx, verilog, yaml, zig**.
//!
//! Grammar crates are restricted to those returning
//! `tree-sitter-language`'s `LanguageFn` (ABI-stable across facade
//! versions). Legacy-binding crates still common on crates.io (markdown,
//! kotlin, dockerfile, sql, scss, vim, vue, svelte, org, bibtex, wgsl,
//! perl, clojure, vimdoc — and latex, whose 0.1.0 publish references a
//! scanner it does not ship) are excluded: their direct facade pins trip
//! cargo's one-copy `links = "tree-sitter"` rule. `tree-sitter-toml-ng`
//! replaces the abandoned `tree-sitter-toml`.
//!
//! # Cache contract
//!
//! Entries are one JSON file per source file under the caller-provided
//! dir, keyed by Blake3 of the canonical path, validated by content hash
//! and length (stored mtime is a fast-path freshness hint, refreshed on
//! hash-preserving touches). Torn, corrupt, outdated, or foreign-version
//! entries are silent misses: the file re-parses and the entry is
//! rewritten. Writes are temp-file + rename, so concurrent analyzers
//! cannot interleave corruption.
//!
//! # Bounded handling
//!
//! Sources over [`parse::MAX_SOURCE_BYTES`] (64 MiB) are a typed
//! [`AstError::FileTooLarge`]; diagnostics cap at
//! [`parse::MAX_DIAGNOSTICS`] with a truncation flag; the diagnostic walk
//! is iterative (stack-safe on adversarially deep trees); invalid UTF-8
//! never panics — names convert lossily.

pub mod analyze;
pub mod cache;
pub mod error;
pub mod grammar;
pub mod language;
pub mod parse;
pub mod tags;
pub mod walk;

pub use analyze::{Analysis, AnalyzeOutcome, Analyzer, CacheStatus};
pub use error::AstError;
pub use language::LanguageId;
pub use parse::{Diagnostic, DiagnosticKind, MAX_DIAGNOSTICS, MAX_SOURCE_BYTES, ParseResult};
pub use tags::{Definition, Point, Reference, Span, SymbolKind, TagTable};
