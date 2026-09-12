//! Stale-state and adversarial cache coverage (plan todo 14): corrupt /
//! outdated entries re-parse silently, deleted caches rebuild, unsupported
//! inputs degrade, oversize sources are typed errors.

use harness_ast::analyze::{AnalyzeOutcome, Analyzer, CacheStatus};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const RUST_SRC: &[u8] = b"fn alpha() {}\nfn beta() {}\n";

fn write_src(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, bytes).unwrap_or_else(|e| panic!("{e}"));
    path
}

fn analyzed(outcome: AnalyzeOutcome) -> harness_ast::analyze::Analysis {
    match outcome {
        AnalyzeOutcome::Analyzed(a) => *a,
        AnalyzeOutcome::NoGrammar { path, .. } => panic!("no grammar for {}", path.display()),
    }
}
#[test]
fn corrupt_cache_file_reparses_silently() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let cache_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);
    let analyzer = Analyzer::with_cache_dir(cache_dir.path());
    analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );

    let mut torn = b"{ torn garbage ".to_vec();
    torn.extend_from_slice(&[0x00, 0xFF]);
    for entry in fs::read_dir(cache_dir.path())
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
    {
        if entry.path().extension().is_some_and(|e| e == "json") {
            fs::write(entry.path(), &torn).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    let after = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(
        after.cache,
        CacheStatus::Miss,
        "torn cache silently re-parses"
    );
    let names: Vec<&str> = after.definitions.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        ["alpha", "beta"],
        "re-parse restores the tables :: {names:?}"
    );
}

#[test]
fn outdated_cache_version_is_a_miss() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let cache_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);
    let analyzer = Analyzer::with_cache_dir(cache_dir.path());
    analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );

    for entry in fs::read_dir(cache_dir.path())
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
    {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "json") {
            let text = fs::read_to_string(&p).unwrap_or_else(|e| panic!("{e}"));
            let downgraded = text.replace("\"v\":1", "\"v\":999");
            fs::write(&p, downgraded).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    let after = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(
        after.cache,
        CacheStatus::Miss,
        "future-version entry is ignored"
    );
    let again = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(
        again.cache,
        CacheStatus::Hit,
        "and re-stored under the current version"
    );
}

#[test]
fn deleted_cache_dir_rebuilds() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let cache_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);

    {
        let analyzer = Analyzer::with_cache_dir(cache_dir.path());
        analyzed(
            analyzer
                .analyze_path(&path)
                .unwrap_or_else(|e| panic!("{e}")),
        );
    }
    fs::remove_dir_all(cache_dir.path()).unwrap_or_else(|e| panic!("{e}"));

    let analyzer = Analyzer::with_cache_dir(cache_dir.path());
    let after = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(after.cache, CacheStatus::Miss);
    let names: Vec<&str> = after.definitions.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["alpha", "beta"]);
    assert_cache_entry_count(cache_dir.path(), 1);
}

#[test]
fn unsupported_extension_degrades_to_typed_no_grammar_not_error() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "blob.xyzzy", b"some\x00binary\x01stuff");
    let analyzer = Analyzer::new();
    match analyzer
        .analyze_path(&path)
        .unwrap_or_else(|e| panic!("{e}"))
    {
        AnalyzeOutcome::NoGrammar { path: p, language } => {
            assert_eq!(p, path);
            assert_eq!(language, None, "unknown extension -> no language guess");
        }
        AnalyzeOutcome::Analyzed(a) => panic!("must degrade, got analysis: {:?}", a.definitions),
    }
}

#[test]
#[cfg(not(feature = "lang-python"))]
fn known_language_without_compiled_grammar_degrades_with_language_named() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "script.py", b"def f(): pass\n");
    let analyzer = Analyzer::new();
    match analyzer
        .analyze_path(&path)
        .unwrap_or_else(|e| panic!("{e}"))
    {
        AnalyzeOutcome::NoGrammar { path: p, language } => {
            assert_eq!(p, path);
            assert_eq!(language, Some(harness_ast::language::LanguageId::Python));
        }
        AnalyzeOutcome::Analyzed(_) => panic!("must degrade: python is not in the default set"),
    }
}

#[test]
fn oversize_file_is_a_typed_error_from_path_analysis() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(
        &src_dir,
        "big.rs",
        &vec![b' '; harness_ast::MAX_SOURCE_BYTES + 1],
    );
    let analyzer = Analyzer::new();
    let err = analyzer.analyze_path(&path).unwrap_err();
    assert!(
        matches!(&err, harness_ast::AstError::FileTooLarge { path: p, .. } if *p == path),
        "wrong error: {err:?}"
    );
}

fn assert_cache_entry_count(dir: &Path, want: usize) {
    let count = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .count();
    assert_eq!(count, want, "cache entries on disk");
}
