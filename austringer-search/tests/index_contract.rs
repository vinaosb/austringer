//! Core index behaviors (plan todo 17): gitignore exclusion, safe binary
//! skip, lossy UTF-8, bounded snippets, typed query errors, definition-first
//! ranking, and no self-indexing of the index directory.

mod common;

use austringer_search::{CodeIndex, IndexOptions, SearchError};
use common::{
    XorShift64, enable_gitignore, write_adversarial_files, write_bytes, write_file,
    write_synth_corpus,
};

fn temp_pair(label: &str) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let outer = tempfile::tempdir().expect("tempdir");
    let corpus = outer.path().join("corpus");
    let index_dir = outer.path().join(label);
    std::fs::create_dir_all(&corpus).expect("mkdir corpus");
    std::fs::create_dir_all(&index_dir).expect("mkdir index");
    (outer, corpus, index_dir)
}

#[test]
fn definition_ranks_above_references_when_symbol_searched() {
    let (_keep, corpus, index_dir) = temp_pair("idx-defs");
    write_file(
        &corpus,
        "def_site.rs",
        "//! defines the symbol\npub fn alpha_widget(input: &str) -> String {\n    input.to_uppercase()\n}\n",
    );
    write_file(
        &corpus,
        "call_site.rs",
        "let a = alpha_widget(\"one\");\nlet b = alpha_widget(\"two\");\nlet c = alpha_widget(\"three\");\n",
    );

    let index = CodeIndex::open(&index_dir).expect("open");
    index.refresh(&corpus).expect("refresh");
    let hits = index.search("alpha_widget", 5).expect("search");

    assert!(!hits.is_empty(), "must find the symbol");
    assert_eq!(hits[0].path, "def_site.rs", "definition must rank first");
    assert!(
        hits[0].snippet.contains("alpha_widget"),
        "snippet names the symbol"
    );
    let paths: Vec<_> = hits.iter().map(|h| h.path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["def_site.rs", "call_site.rs"],
        "both docs match"
    );
}

#[test]
fn gitignored_paths_are_excluded_from_index() {
    let (_keep, corpus, index_dir) = temp_pair("idx-gitignore");
    enable_gitignore(&corpus);
    write_file(&corpus, "visible.rs", "pub fn visible_marker() {}\n");
    write_file(
        &corpus,
        "ignored_dir/secret.rs",
        "pub fn hidden_marker() {}\n",
    );

    let index = CodeIndex::open(&index_dir).expect("open");
    let report = index.refresh(&corpus).expect("refresh");

    assert_eq!(report.added, 1, "only the visible file is indexed");
    let hits = index.search("hidden_marker", 5).expect("search");
    assert!(hits.is_empty(), "gitignored content must be unreachable");
    let visible = index.search("visible_marker", 5).expect("search");
    assert_eq!(visible.len(), 1, "visible file still indexed");
}

#[test]
fn binary_files_are_skipped_safely_without_corpus_pollution() {
    let (_keep, corpus, index_dir) = temp_pair("idx-binary");
    write_file(&corpus, "text.rs", "pub fn text_marker() {}\n");
    write_adversarial_files(&corpus);

    let index = CodeIndex::open(&index_dir).expect("open");
    let report = index.refresh(&corpus).expect("refresh");

    assert_eq!(
        report.skipped_binary, 1,
        "the NUL blob is counted as skipped"
    );
    let binary_hits = index.search("binary_needle", 5).expect("search");
    assert!(
        binary_hits.is_empty(),
        "binary content must never be searchable"
    );
    let text = index.search("text_marker", 5).expect("search");
    assert_eq!(text.len(), 1, "text files unaffected");
}

#[test]
fn invalid_utf8_files_are_indexed_lossily() {
    let (_keep, corpus, index_dir) = temp_pair("idx-utf8");
    write_adversarial_files(&corpus);

    let index = CodeIndex::open(&index_dir).expect("open");
    index.refresh(&corpus).expect("refresh");

    let hits = index.search("needle_toxic", 5).expect("search");
    assert_eq!(
        hits.len(),
        1,
        "ASCII needle inside invalid-UTF-8 file is found"
    );
    assert_eq!(hits[0].path, "weird_utf8.txt");
}

#[test]
fn huge_single_line_file_is_handled_gracefully() {
    let (_keep, corpus, index_dir) = temp_pair("idx-huge");
    write_adversarial_files(&corpus);

    let index = CodeIndex::open(&index_dir).expect("open");
    let report = index.refresh(&corpus).expect("refresh");

    assert_eq!(report.added, 2, "utf-8 + huge-line files both indexed");
    assert_eq!(report.skipped_binary, 1);
    // A 300 KB tokenless line is one token beyond the tokenizer's
    // long-token filter: the document is present (see `added` above) but
    // BM25 cannot match inside it. Regex search covers this shape —
    // `huge_single_line_snippet_is_bounded` proves grep finds the needle.
    assert!(
        index
            .search("huge_needle_alpha", 5)
            .expect("search")
            .is_empty(),
        "no panic, no partial-token matches, clean index"
    );
}

#[test]
fn invalid_query_syntax_returns_typed_error() {
    let (_keep, corpus, index_dir) = temp_pair("idx-badquery");
    let mut rng = XorShift64::new(0xC0FFEE);
    write_synth_corpus(&corpus, 5, &mut rng);

    let index = CodeIndex::open(&index_dir).expect("open");
    index.refresh(&corpus).expect("refresh");

    for bad in ["fn (", "\"unclosed", "field-does-not-exist:x", "^2.0"] {
        let err = index
            .search(bad, 5)
            .expect_err("malformed query must error");
        assert!(
            matches!(err, SearchError::InvalidQuery(_)),
            "query {bad:?} must be InvalidQuery, got {err:?}"
        );
    }
}

#[test]
fn oversize_files_are_skipped_not_indexed() {
    let (_keep, corpus, index_dir) = temp_pair("idx-oversize");
    write_file(&corpus, "small.rs", "pub fn small_marker() {}\n");
    write_bytes(corpus.join("big.rs").parent().unwrap(), "big.rs", &[]);
    std::fs::write(
        corpus.join("big.rs"),
        format!("{} oversize_marker", "y".repeat(700_000)),
    )
    .expect("write oversize");

    let opts = IndexOptions {
        max_file_bytes: 65_536,
        ..IndexOptions::default()
    };
    let index = CodeIndex::open(&index_dir).expect("open");
    let report = index.refresh_with(&corpus, &opts).expect("refresh");

    assert_eq!(report.skipped_oversize, 1);
    assert!(
        index
            .search("oversize_marker", 5)
            .expect("search")
            .is_empty()
    );
    assert_eq!(index.search("small_marker", 5).expect("search").len(), 1);
}

#[test]
fn index_directory_inside_root_is_never_indexed() {
    let (_keep, corpus, _index_dir) = temp_pair("unused");
    // Non-hidden index directory directly inside the corpus root.
    let index_dir = corpus.join("index-store");
    std::fs::create_dir_all(&index_dir).expect("mkdir index");

    let mut rng = XorShift64::new(0x5EED);
    write_synth_corpus(&corpus, 10, &mut rng);

    let index = CodeIndex::open(&index_dir).expect("open");
    let first = index.refresh(&corpus).expect("refresh");
    assert_eq!(first.added, 10, "corpus files only, no index files");
    assert_eq!(first.docs_indexed, 10);

    // Idempotent second pass: if index files leaked into the corpus the
    // walker would keep finding new "changed" files forever.
    let second = index.refresh(&corpus).expect("refresh");
    assert_eq!(second.touched(), 0, "index files must not feed back");
    assert_eq!(second.docs_indexed, 10);
}
