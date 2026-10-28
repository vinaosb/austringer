//! Pure-logic contract tests: config table + extension routing, backoff
//! math, URI round-trips, UTF-16 position conversion, and the compact
//! (<25-token-approximation) rendering budget.

use std::path::PathBuf;
use std::time::Duration;

use austringer_lsp::config::{Backoff, Language, LspConfig, ServerCommand, language_for_extension};
use austringer_lsp::error::DegradeReason;
use austringer_lsp::position::utf16_col_to_utf8;
use austringer_lsp::provider::{Answer, HoverText, Location, SEARCH_FALLBACK_HINT};
use austringer_lsp::render::{LOCATION_BUDGET_CHARS, render_locations};
use austringer_lsp::uri::{file_to_uri, uri_to_file};

fn root() -> PathBuf {
    std::env::temp_dir().join("austringer-lsp-pure-root")
}

#[test]
fn default_table_covers_the_five_planned_languages() {
    let cfg = LspConfig::new(root());
    for lang in [
        Language::Rust,
        Language::Go,
        Language::TypeScript,
        Language::C,
        Language::Python,
    ] {
        assert!(
            cfg.servers.contains_key(&lang),
            "default table missing {lang:?}"
        );
    }
    assert_eq!(
        cfg.servers[&Language::Rust],
        ServerCommand {
            program: "rust-analyzer".into(),
            args: vec![],
        }
    );
    // tsserver speaks no LSP natively; the language-server wrapper does.
    assert_eq!(
        cfg.servers[&Language::TypeScript].program,
        "typescript-language-server"
    );
    assert_eq!(cfg.servers[&Language::TypeScript].args, vec!["--stdio"]);
}

#[test]
fn extension_routing_covers_all_table_languages() {
    assert_eq!(language_for_extension("rs"), Some(Language::Rust));
    assert_eq!(language_for_extension("go"), Some(Language::Go));
    assert_eq!(language_for_extension("ts"), Some(Language::TypeScript));
    assert_eq!(language_for_extension("tsx"), Some(Language::TypeScript));
    assert_eq!(language_for_extension("js"), Some(Language::TypeScript));
    assert_eq!(language_for_extension("c"), Some(Language::C));
    assert_eq!(language_for_extension("h"), Some(Language::C));
    assert_eq!(language_for_extension("cpp"), Some(Language::Cpp));
    assert_eq!(language_for_extension("py"), Some(Language::Python));
    assert_eq!(language_for_extension("exe"), None);
    assert_eq!(
        language_for_extension("RS"),
        Some(Language::Rust),
        "case-insensitive"
    );
}

#[test]
fn backoff_is_plain_exponential_and_capped() {
    let b = Backoff {
        initial: Duration::from_millis(500),
        multiplier: 2.0,
        max: Duration::from_secs(30),
    };
    assert_eq!(b.wait_for(1), Duration::from_millis(500));
    assert_eq!(b.wait_for(2), Duration::from_secs(1));
    assert_eq!(b.wait_for(3), Duration::from_secs(2));
    assert_eq!(b.wait_for(7), Duration::from_secs(30), "capped");
    assert_eq!(b.wait_for(50), Duration::from_secs(30), "still capped");
    assert_eq!(
        b.wait_for(0),
        Duration::ZERO,
        "no backoff before first crash"
    );
}

#[test]
fn uri_round_trips_windows_paths() {
    let p = PathBuf::from(r"C:\Users\vsbb1\repo\src\lib.rs");
    let uri = file_to_uri(&p).expect("encodes");
    assert!(uri.starts_with("file:///"), "uri = {uri}");
    let back = uri_to_file(&uri);
    assert_eq!(
        back.to_string_lossy().replace('\\', "/"),
        p.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn uri_percent_encodes_spaces_and_unicode() {
    let p = PathBuf::from(r"C:\temp dir\ünïcode.rs");
    let uri = file_to_uri(&p).expect("encodes");
    assert!(!uri.contains(' '), "space must be encoded: {uri}");
    assert!(uri.contains("%C3%BC"), "ü must be percent-encoded: {uri}");
    let back = uri_to_file(&uri);
    assert_eq!(
        back.to_string_lossy().replace('\\', "/"),
        p.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn utf16_columns_convert_to_utf8_columns() {
    // "äbc" — ä is BMP: 1 char, 1 UTF-16 unit, 2 UTF-8 bytes. Units are
    // char-aligned here, so this checks the accounting.
    let line = "äbc";
    assert_eq!(utf16_col_to_utf8(line, 0), 0);
    assert_eq!(utf16_col_to_utf8(line, 1), 1, "after ä (1 unit) -> 1 char");
    assert_eq!(utf16_col_to_utf8(line, 2), 2);
    // Emoji: non-BMP, 1 char, 2 UTF-16 units, 4 UTF-8 bytes — the real
    // conversion case.
    let emoji = "a😀b";
    assert_eq!(utf16_col_to_utf8(emoji, 1), 1, "after 'a'");
    assert_eq!(
        utf16_col_to_utf8(emoji, 3),
        2,
        "after emoji (2 units) -> char 2"
    );
    // Surrogate-pair first: pointing INTO the pair lands after it.
    assert_eq!(utf16_col_to_utf8("😀x", 1), 1);
    // Out-of-range clamps to the line end (untrusted server data never panics).
    assert_eq!(utf16_col_to_utf8("ab", 99), 2);
}

#[test]
fn render_locations_respects_the_token_budget() {
    let locs: Vec<Location> = (0..40)
        .map(|i| Location {
            file: format!("C:/repo/src/mod_{i:02}/deeply/nested/file_name.rs").into(),
            line: 100 + i,
            column: 4,
        })
        .collect();
    let text = render_locations(&root(), &locs);
    assert!(
        text.chars().count() <= LOCATION_BUDGET_CHARS,
        "rendered {} chars over budget {}",
        text.chars().count(),
        LOCATION_BUDGET_CHARS
    );
    assert!(
        text.contains("+"),
        "truncation marker expected when over budget: {text}"
    );
}

#[test]
fn render_locations_is_relative_to_root_when_inside() {
    let loc = Location {
        file: root().join("src/lib.rs"),
        line: 3,
        column: 8,
    };
    let text = render_locations(&root(), std::slice::from_ref(&loc));
    assert_eq!(text, "src/lib.rs:3:8");
}

#[test]
fn hover_text_is_bounded_at_construction() {
    let long = "x".repeat(10_000);
    let h = HoverText::new(&long);
    assert!(h.as_str().chars().count() < 10_000);
    assert!(h.as_str().ends_with('…'), "truncation ellipsis expected");
}

#[test]
fn degraded_answer_names_the_search_fallback() {
    let answer = Answer::DegradedToSearch {
        reason: DegradeReason::RestartBudgetExhausted { crashes: 2 },
        hint: SEARCH_FALLBACK_HINT,
    };
    let Answer::DegradedToSearch { hint, .. } = answer else {
        panic!("expected the degraded variant");
    };
    assert!(hint.contains("lexical search"), "hint = {hint}");
}
