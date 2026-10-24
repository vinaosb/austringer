//! Grep integration (plan todo 17): ripgrep-family streaming regex search
//! with ignore rules, bounded windowed snippets, hit merging/dedupe, caps,
//! safe binary skip, and typed regex errors.

mod common;

use common::{enable_gitignore, write_adversarial_files, write_bytes, write_file};
use harness_search::{GrepHit, GrepOptions, SearchError, grep_stream};

fn default_hits(root: &std::path::Path, pattern: &str) -> Vec<GrepHit> {
    grep_stream(root, pattern, &GrepOptions::default())
        .expect("compile pattern")
        .collect()
}

#[test]
fn grep_streams_ranked_windows_with_snippets() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    write_file(root, "a.rs", "one\nNEEDLE_GAMMA here\nthree\n");
    write_file(root, "b.rs", "alpha\nbeta NEEDLE_GAMMA\n");

    let hits = default_hits(root, "NEEDLE_GAMMA");
    assert_eq!(hits.len(), 2, "one hit per file");
    // Equal-score ties stream in readdir order, which differs across
    // filesystems; sort so the asserted order is deterministic.
    let mut hits = hits;
    hits.sort_by(|a, b| (a.line_start, &a.path).cmp(&(b.line_start, &b.path)));
    assert_eq!(hits[0].path, "a.rs");
    assert_eq!(hits[0].line_start, 2);
    assert_eq!(hits[0].line_end, 2);
    assert!(hits[0].snippet.contains("NEEDLE_GAMMA"), "snippet content");
    assert_eq!(hits[1].path, "b.rs");
    assert_eq!(hits[1].line_start, 2);
}

#[test]
fn adjacent_matches_merge_into_single_window() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    write_file(
        root,
        "merged.rs",
        "hit MERGE_ALPHA\nhit MERGE_ALPHA\nhit MERGE_ALPHA\n\
         filler line\nhit MERGE_ALPHA\nfiller\nfiller\nfiller\nhit MERGE_ALPHA\n",
    );

    let hits = default_hits(root, "MERGE_ALPHA");
    assert_eq!(hits.len(), 2, "nearby matches dedupe to windows");
    assert_eq!(
        (hits[0].line_start, hits[0].line_end),
        (1, 5),
        "merged span"
    );
    assert_eq!(
        (hits[1].line_start, hits[1].line_end),
        (9, 9),
        "isolated match"
    );
}

#[test]
fn hit_cap_limits_returned_entries() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    let mut body = String::new();
    // A match every 3rd line: gaps of 2 exceed the default merge gap, so
    // each match stays its own window.
    for line in 1..=300u32 {
        body.push_str(if line % 3 == 0 {
            "x CAP_NEEDLE"
        } else {
            "x plain"
        });
        body.push('\n');
    }
    write_file(root, "cap.txt", &body);

    let opts = GrepOptions {
        max_hits: 5,
        ..GrepOptions::default()
    };
    let hits: Vec<GrepHit> = grep_stream(root, "CAP_NEEDLE", &opts)
        .expect("compile")
        .take(10)
        .collect();
    assert_eq!(hits.len(), 5, "cap stops the stream early");
}

#[test]
fn binary_files_yield_no_grep_hits() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    write_adversarial_files(root);

    let hits = default_hits(root, "binary_needle");
    assert!(hits.is_empty(), "NUL blob yields zero hits, no panic");

    let toxic = default_hits(root, "needle_toxic");
    assert_eq!(toxic.len(), 1, "invalid UTF-8 still greps lossily");
    assert_eq!(toxic[0].path, "weird_utf8.txt");
    assert!(toxic[0].snippet.contains("needle_toxic"));
}

#[test]
fn huge_single_line_snippet_is_bounded() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    write_adversarial_files(root);

    let hits = default_hits(root, "huge_needle_alpha");
    assert_eq!(hits.len(), 1, "huge single line matched");
    assert_eq!(hits[0].line_start, 1);
    assert!(
        hits[0].snippet.len() <= 512,
        "snippet bounded to {} bytes",
        hits[0].snippet.len()
    );
}

#[test]
fn invalid_regex_returns_typed_error() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    write_file(root, "x.txt", "irrelevant\n");

    for bad in ["(", "*", "(?P<", "[a-"] {
        let err = grep_stream(root, bad, &GrepOptions::default())
            .err()
            .unwrap_or_else(|| panic!("pattern {bad:?} must be rejected"));
        assert!(
            matches!(err, SearchError::InvalidRegex(_)),
            "pattern {bad:?} must be InvalidRegex, got {err:?}"
        );
    }
}

#[test]
fn anchored_literal_and_case_modes_supported() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    write_file(
        root,
        "modes.rs",
        "pub fn real() {}\n  pub fn indented() {}\ncall fn(42);\n",
    );

    let anchored = default_hits(root, "^pub fn");
    assert_eq!(anchored.len(), 1, "anchor excludes indented line");
    assert_eq!(anchored[0].line_start, 1);

    let literal_opts = GrepOptions {
        literal: true,
        ..GrepOptions::default()
    };
    let literal = grep_stream(root, "fn(42", &literal_opts)
        .expect("compile literal")
        .collect::<Vec<_>>();
    assert_eq!(literal.len(), 1, "parenthesis is literal, not grouping");
    assert_eq!(literal[0].line_start, 3);

    assert_eq!(
        default_hits(root, "REAL").len(),
        0,
        "default is case-sensitive"
    );
    let ci_opts = GrepOptions {
        case_insensitive: true,
        ..GrepOptions::default()
    };
    let ci = grep_stream(root, "REAL", &ci_opts)
        .expect("compile case-insensitive")
        .collect::<Vec<_>>();
    assert_eq!(ci.len(), 1, "opt-in case-insensitive matches");
}

#[test]
fn gitignored_paths_are_excluded_from_grep() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    enable_gitignore(root);
    write_file(root, "visible.rs", "VISIBLE_NEEDLE\n");
    write_file(root, "ignored_dir/secret.rs", "HIDDEN_NEEDLE\n");

    assert_eq!(default_hits(root, "HIDDEN_NEEDLE").len(), 0);
    assert_eq!(default_hits(root, "VISIBLE_NEEDLE").len(), 1);
}

#[test]
fn grep_skips_oversize_files_without_panicking() {
    let corpus = tempfile::tempdir().expect("tempdir");
    let root = corpus.path();
    let small = "z".repeat(64);
    let big = format!("{} OVERSIZE_NEEDLE", "z".repeat(700_000));
    write_file(root, "small.txt", &small);
    write_bytes(root, "big.txt", big.as_bytes());

    let opts = GrepOptions {
        max_file_bytes: 65_536,
        ..GrepOptions::default()
    };
    let hits: Vec<GrepHit> = grep_stream(root, "OVERSIZE_NEEDLE", &opts)
        .expect("compile")
        .collect();
    assert!(hits.is_empty(), "oversize file skipped, stream completes");
}
