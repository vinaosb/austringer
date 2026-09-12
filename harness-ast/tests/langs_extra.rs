//! Opt-in 50-language smoke (plan todo 14 QA): with `--features
//! langs-extra` every extra-language grammar compiles, its tags query
//! compiles, and a minimal real fixture parses to the expected symbol
//! content. Not run in the default suite by design (grammar C-compilation
//! cost); verified once per change of the feature surface.

#![cfg(feature = "langs-extra")]

use harness_ast::Analyzer;
use harness_ast::analyze::AnalyzeOutcome;
use harness_ast::language::LanguageId;
use harness_ast::tags::SymbolKind;
use std::fs;
use tempfile::TempDir;

struct Fixture {
    language: LanguageId,
    file_name: &'static str,
    source: &'static str,
    expected_defs: Vec<(&'static str, SymbolKind)>,
}

fn fixture(
    language: LanguageId,
    file_name: &'static str,
    source: &'static str,
    expected_defs: &[(&'static str, SymbolKind)],
) -> Fixture {
    Fixture {
        language,
        file_name,
        source,
        expected_defs: expected_defs.to_vec(),
    }
}

fn fixtures() -> Vec<Fixture> {
    use LanguageId as L;
    const fn other() -> SymbolKind {
        SymbolKind::Other(String::new())
    }
    vec![
        fixture(
            L::Python,
            "add.py",
            "def add(a, b):\n    return helper(a) + b\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::JavaScript,
            "add.js",
            "function add(a, b) {\n  return calc(a);\n}\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::TypeScript,
            "area.ts",
            "interface Shape { sides: number }\nfunction area(s: Shape): number {\n  return s.sides;\n}\n",
            &[
                ("Shape", SymbolKind::Interface),
                ("area", SymbolKind::Function),
            ],
        ),
        fixture(
            L::Tsx,
            "app.tsx",
            "function App() {\n  return null;\n}\n",
            &[("App", SymbolKind::Function)],
        ),
        fixture(
            L::Go,
            "add.go",
            "package main\n\nfunc add(a int, b int) int {\n\treturn a + b\n}\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::C,
            "add.c",
            "int add(int a, int b) {\n  return a + b;\n}\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Cpp,
            "point.cpp",
            "class Point {\npublic:\n  int x;\n};\nint main() {\n  return 0;\n}\n",
            &[("Point", SymbolKind::Class), ("main", SymbolKind::Function)],
        ),
        fixture(
            L::CSharp,
            "calc.cs",
            "namespace MathNs {\n  public class Calc {\n    public int Add(int a, int b) { return a + b; }\n  }\n}\n",
            &[
                ("MathNs", SymbolKind::Module),
                ("Calc", SymbolKind::Class),
                ("Add", SymbolKind::Method),
            ],
        ),
        fixture(
            L::Java,
            "main.java",
            "public class Main {\n  public static void main(String[] args) { }\n}\n",
            &[("Main", SymbolKind::Class), ("main", SymbolKind::Method)],
        ),
        fixture(
            L::Swift,
            "add.swift",
            "func add(_ a: Int, _ b: Int) -> Int {\n  return a + b\n}\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Scala,
            "main.scala",
            "object Main {\n  def add(a: Int, b: Int): Int = a + b\n}\n",
            &[("Main", SymbolKind::Class), ("add", SymbolKind::Function)],
        ),
        fixture(
            L::Ruby,
            "add.rb",
            "module M\ndef add(a, b)\n  helper(a)\n  a + b\nend\nend\n",
            &[("M", SymbolKind::Module), ("add", SymbolKind::Method)],
        ),
        fixture(
            L::Php,
            "add.php",
            "<?php\nfunction add($a, $b) { return $a + $b; }\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Lua,
            "add.lua",
            "local function add(a, b)\n  return helper(a) + b\nend\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::R,
            "add.R",
            "add <- function(a, b) {\n  helper(a)\n}\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Julia,
            "add.jl",
            "function add(a, b)\n  return a + b\nend\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Haskell,
            "add.hs",
            "add :: Int -> Int -> Int\nadd a b = a + b\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Ocaml,
            "add.ml",
            "let add a b = a + b\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::OcamlInterface,
            "ver.mli",
            "val version : int\n",
            &[("version", SymbolKind::Function)],
        ),
        fixture(
            L::Elixir,
            "math.ex",
            "defmodule Math do\n  def add(a, b), do: a + b\nend\n",
            &[("Math", SymbolKind::Module), ("add", SymbolKind::Function)],
        ),
        fixture(
            L::Erlang,
            "math.erl",
            "-module(math).\nadd(A, B) -> A + B.\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Yaml,
            "job.yaml",
            "name: harness\njobs:\n  build:\n    runs: 2\n",
            &[
                ("name", SymbolKind::Property),
                ("runs", SymbolKind::Property),
            ],
        ),
        fixture(
            L::Html,
            "page.html",
            "<div class=\"a\">\n  <p id=\"x\">t</p>\n</div>\n",
            &[("div", other()), ("p", other())],
        ),
        fixture(
            L::Css,
            "btn.css",
            ".btn-primary {\n  color: red;\n}\n#app {\n  margin: 0;\n}\n",
            &[
                ("btn-primary", SymbolKind::Class),
                ("app", SymbolKind::Constant),
                ("color", SymbolKind::Property),
            ],
        ),
        fixture(
            L::Graphql,
            "schema.graphql",
            "type Query {\n  hero: Character\n}\n",
            &[("Query", SymbolKind::Type), ("hero", SymbolKind::Property)],
        ),
        fixture(
            L::Zig,
            "add.zig",
            "fn add(a: u32, b: u32) u32 {\n  return a + b;\n}\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Nix,
            "conf.nix",
            "{ pkgs = import <nixpkgs> {}; }\n",
            &[("pkgs", SymbolKind::Variable)],
        ),
        fixture(
            L::Make,
            "Makefile",
            "CC = gcc\nall:\n\techo hi\n",
            &[("CC", SymbolKind::Variable)],
        ),
        fixture(
            L::Cmake,
            "cmake.cmake",
            "cmake_minimum_required(VERSION 3.10)\nproject(Hello)\n",
            &[],
        ),
        fixture(
            L::Hcl,
            "main.tf",
            "resource \"aws_instance\" \"web\" {\n  ami = \"abc\"\n}\n",
            &[
                ("aws_instance", SymbolKind::Section),
                ("ami", SymbolKind::Variable),
            ],
        ),
        fixture(
            L::Dart,
            "point.dart",
            "class Point {\n  int x = 0;\n}\n",
            &[("Point", SymbolKind::Class)],
        ),
        fixture(
            L::Elm,
            "add.elm",
            "add : Int -> Int -> Int\nadd a b = a + b\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Rst,
            "doc.rst",
            "Title\n=====\n\nSome text.\n",
            &[("Title", SymbolKind::Section)],
        ),
        fixture(
            L::Solidity,
            "counter.sol",
            "pragma solidity ^0.8.0;\ncontract Counter {\n  function inc() public returns (uint) { return 1; }\n}\n",
            &[("Counter", other()), ("inc", SymbolKind::Function)],
        ),
        fixture(
            L::Scheme,
            "add.scm",
            "(define (add x y) (+ x y))\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::CommonLisp,
            "add.lisp",
            "(defun add (x y) (+ x y))\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Elisp,
            "init.el",
            "(defun add (x y) (+ x y))\n(defmacro twice (x) x)\n",
            &[("add", SymbolKind::Function), ("twice", SymbolKind::Macro)],
        ),
        fixture(
            L::Powershell,
            "greet.ps1",
            "function Greet {\n  param($name)\n  Write-Output \"hi\"\n}\n",
            &[("Greet", SymbolKind::Function)],
        ),
        fixture(
            L::Groovy,
            "main.groovy",
            "class Main {\n  def add(int a, int b) { a + b }\n}\n",
            &[("Main", SymbolKind::Class), ("add", SymbolKind::Method)],
        ),
        fixture(
            L::Nim,
            "add.nim",
            "proc add(a: int, b: int): int = a + b\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::ObjC,
            "foo.mm",
            "@interface Foo : NSObject\n@end\n@implementation Foo\n- (void)bar { }\n@end\n",
            &[("bar", SymbolKind::Method)],
        ),
        fixture(
            L::Verilog,
            "counter.v",
            "module counter(input clk);\nendmodule\n",
            &[("counter", SymbolKind::Module)],
        ),
        fixture(
            L::FSharp,
            "add.fs",
            "let add x y = x + y\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Fortran,
            "add.f90",
            "function add(a, b) result(c)\n  integer :: a, b, c\n  c = a + b\nend function add\n",
            &[("add", SymbolKind::Function)],
        ),
        fixture(
            L::Matlab,
            "add.m",
            "function y = add(a, b)\n  y = a + b;\nend\n",
            &[("add", SymbolKind::Function)],
        ),
    ]
}

#[test]
fn extra_language_surface_parses_and_tags_fixtures() {
    let dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let analyzer = Analyzer::new();
    let fixtures = fixtures();
    assert_eq!(
        fixtures.len(),
        45,
        "file-based langs-extra fixtures (regex is covered by bytes below)"
    );

    let mut failures = Vec::new();
    for fixture in &fixtures {
        let path = dir.path().join(fixture.file_name);
        fs::write(&path, fixture.source).unwrap_or_else(|e| panic!("{e}"));
        let analysis = match analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}"))
        {
            AnalyzeOutcome::Analyzed(a) => *a,
            AnalyzeOutcome::NoGrammar { language, .. } => {
                failures.push(format!(
                    "{}: grammar not compiled (expected {})",
                    fixture.file_name,
                    language
                        .map(|l| l.name().to_owned())
                        .unwrap_or_else(|| "?".into())
                ));
                continue;
            }
        };
        if analysis.language != fixture.language {
            failures.push(format!(
                "{}: detected {:?}, want {:?}",
                fixture.file_name, analysis.language, fixture.language
            ));
            continue;
        }
        let found: Vec<String> = analysis
            .definitions
            .iter()
            .map(|d| format!("{}:{:?}", d.name, d.kind))
            .collect();
        for (want_name, want_kind) in &fixture.expected_defs {
            let ok = analysis.definitions.iter().any(|d| {
                let name_ok = match want_kind {
                    SymbolKind::Other(_) => d.name.contains(want_name),
                    _ => &d.name == want_name,
                };
                name_ok && std::mem::discriminant(&d.kind) == std::mem::discriminant(want_kind)
            });
            if !ok {
                failures.push(format!(
                    "{}: def {want_name:?} ({want_kind:?}) missing :: {found:?}",
                    fixture.file_name
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} fixture failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn regex_language_parses_via_explicit_language_only() {
    // Regex has no file extension mapping by design; hosts opt in with an
    // explicit LanguageId (analyze_bytes), so it cannot hijack extensions.
    let analysis = Analyzer::new()
        .analyze_bytes(LanguageId::Regex, b"^[a-z]+(?:_[a-z]+)*$\n".to_vec())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(analysis.language, LanguageId::Regex);
    assert!(!analysis.has_error, ":: {:?}", analysis.diagnostics);
    assert!(analysis.definitions.is_empty(), "regex names nothing");
}

#[test]
fn default_languages_still_work_under_langs_extra() {
    let dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = dir.path().join("lib.rs");
    fs::write(&path, b"fn alpha() {}\n").unwrap_or_else(|e| panic!("{e}"));
    let analysis = match Analyzer::new()
        .analyze_path(&path)
        .unwrap_or_else(|e| panic!("{e}"))
    {
        AnalyzeOutcome::Analyzed(a) => *a,
        AnalyzeOutcome::NoGrammar { .. } => panic!("rust missing"),
    };
    assert_eq!(analysis.definitions.len(), 1);
    assert_eq!(analysis.definitions[0].name, "alpha");
}
