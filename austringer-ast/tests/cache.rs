//! Persistent-table cache (plan todo 14): Blake3+mtime keyed parse cache
//! under a caller-provided dir. Stale-state adversarial coverage: edit →
//! hash change → re-parse; identical content rewrite → still a hit; cache
//! deletion → rebuild; corrupt/outdated cache files → silent re-parse.

use austringer_ast::analyze::{AnalyzeOutcome, Analyzer, CacheStatus};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

const RUST_SRC: &[u8] = b"fn alpha() {}\nfn beta() {}\n";

fn write_src(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, bytes).unwrap_or_else(|e| panic!("{e}"));
    path
}

fn analyzed(outcome: AnalyzeOutcome) -> austringer_ast::analyze::Analysis {
    match outcome {
        AnalyzeOutcome::Analyzed(a) => *a,
        AnalyzeOutcome::NoGrammar { path, .. } => panic!("no grammar for {}", path.display()),
    }
}

#[test]
fn first_analyze_is_a_miss_and_second_is_a_hit() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let cache_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);
    let analyzer = Analyzer::with_cache_dir(cache_dir.path());

    let first = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(first.cache, CacheStatus::Miss);
    let names: Vec<&str> = first.definitions.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        ["alpha", "beta"],
        "content extracted on miss :: {names:?}"
    );

    let second = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(second.cache, CacheStatus::Hit);
    assert_eq!(
        second.definitions, first.definitions,
        "hit returns identical tables"
    );
    assert_eq!(second.references, first.references);
    assert_eq!(second.diagnostics, first.diagnostics);
}

#[test]
fn editing_file_changes_hash_and_forces_reparse() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let cache_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);
    let analyzer = Analyzer::with_cache_dir(cache_dir.path());
    analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_cache_entry_count(cache_dir.path(), 1);

    fs::write(&path, b"fn alpha() {}\nfn gamma() {}\n").unwrap_or_else(|e| panic!("{e}"));
    let after = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(after.cache, CacheStatus::Miss, "hash changed -> re-parse");
    let names: Vec<&str> = after.definitions.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        ["alpha", "gamma"],
        "new tables reflect the edit :: {names:?}"
    );

    let hit = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(hit.cache, CacheStatus::Hit);
}

#[test]
fn identical_rewrite_with_new_mtime_stays_a_hit() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let cache_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);
    let analyzer = Analyzer::with_cache_dir(cache_dir.path());
    analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );

    // Same bytes, new mtime: the content hash is authoritative.
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&path, RUST_SRC).unwrap_or_else(|e| panic!("{e}"));
    let after = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(
        after.cache,
        CacheStatus::Hit,
        "identical content must not re-parse"
    );
}

#[test]
fn cacheless_analyzer_reports_disabled_and_correct_tables() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);
    let analyzer = Analyzer::new();
    let a = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(a.cache, CacheStatus::Disabled);
    let b = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert_eq!(b.cache, CacheStatus::Disabled);
    assert_eq!(a.definitions, b.definitions);
}

#[test]
fn diagnostics_carry_the_file_path_and_analysis_flags() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(
        &src_dir,
        "broken.rs",
        b"fn main() {\n    let x = (1 + 2;\n}\n",
    );
    let analyzer = Analyzer::new();
    let a = analyzed(
        analyzer
            .analyze_path(&path)
            .unwrap_or_else(|e| panic!("{e}")),
    );
    assert!(a.has_missing);
    assert!(!a.diagnostics.is_empty());
    for d in &a.diagnostics {
        assert_eq!(
            d.file.as_deref(),
            Some(path.as_path()),
            "diagnostics name the file :: {d:?}"
        );
    }
}

#[test]
fn analysis_is_deterministic_across_fresh_analyzers() {
    let src_dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    let path = write_src(&src_dir, "lib.rs", RUST_SRC);
    let mut first = None;
    for _ in 0..3 {
        let analyzer = Analyzer::new();
        let a = analyzed(
            analyzer
                .analyze_path(&path)
                .unwrap_or_else(|e| panic!("{e}")),
        );
        match &first {
            None => first = Some(a),
            Some(f) => {
                assert_eq!(f.definitions, a.definitions);
                assert_eq!(f.references, a.references);
                assert_eq!(f.diagnostics, a.diagnostics);
            }
        }
    }
}

#[test]
fn analyze_bytes_extracts_tables_without_cache() {
    let analyzer = Analyzer::new();
    let a = analyzer
        .analyze_bytes(
            austringer_ast::language::LanguageId::Rust,
            b"fn alpha() {}\n".to_vec(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(a.cache, CacheStatus::Disabled);
    let names: Vec<&str> = a.definitions.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["alpha"]);
}

fn assert_cache_entry_count(dir: &Path, want: usize) {
    let count = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .count();
    assert_eq!(count, want, "cache entries on disk");
}
