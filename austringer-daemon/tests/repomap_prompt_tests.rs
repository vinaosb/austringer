//! Session prompt composition with the repository map: bounded
//! hydration over the frozen base prompt — gitignore-aware,
//! deterministic, never fatal to spawn.

use std::fs;
use std::path::Path;

use austringer_daemon::engine::SESSION_SYSTEM_PROMPT;
use austringer_daemon::repomap::compose_system_prompt;
use tempfile::TempDir;

fn write(dir: &TempDir, relative: &str, bytes: &[u8]) {
    let path = dir.path().join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
    }
    fs::write(path, bytes).unwrap_or_else(|e| panic!("{e}"));
}

/// Workspace fixture: three grammar files (one nested), one gitignored
/// source file, one non-grammar file.
fn fixture() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    write(&dir, ".gitignore", b"secret/\n");
    write(&dir, "alpha.rs", b"pub fn alpha_fn() {}\n");
    write(&dir, "beta.rs", b"pub fn beta_fn() { alpha_fn() }\n");
    write(&dir, "nested/gamma.rs", b"pub fn gamma_fn() {}\n");
    write(&dir, "secret/hidden.rs", b"pub fn secret_fn() {}\n");
    write(&dir, "notes.txt", b"plainly not source\n");
    dir
}

#[test]
fn composed_prompt_carries_the_map_and_skips_gitignored_symbols() {
    let dir = fixture();
    let cache = TempDir::new().expect("cache dir");
    let prompt = compose_system_prompt(dir.path(), cache.path(), 1000);
    assert!(
        prompt.starts_with(SESSION_SYSTEM_PROMPT),
        "the frozen base prompt prefixes the composed prompt"
    );
    assert!(prompt.contains("<repomap>"), "map block opened");
    assert!(prompt.contains("</repomap>"), "map block closed");
    assert!(prompt.contains("alpha_fn"), "fixture symbol mapped");
    assert!(
        !prompt.contains("secret_fn"),
        "gitignored symbols never hydrate"
    );
}

#[test]
fn zero_budget_disables_the_block() {
    let dir = fixture();
    let cache = TempDir::new().expect("cache dir");
    let prompt = compose_system_prompt(dir.path(), cache.path(), 0);
    assert_eq!(prompt, SESSION_SYSTEM_PROMPT);
}

#[test]
fn composition_is_byte_identical_across_independent_cache_dirs() {
    let dir = fixture();
    let cache_a = TempDir::new().expect("cache a");
    let cache_b = TempDir::new().expect("cache b");
    let first = compose_system_prompt(dir.path(), cache_a.path(), 1000);
    let second = compose_system_prompt(dir.path(), cache_b.path(), 1000);
    assert_eq!(
        first, second,
        "determinism is content-derived, not cache-derived"
    );
    assert!(
        first.contains("<repomap>"),
        "the compared texts were non-trivial"
    );
}

#[test]
fn empty_workspace_yields_the_plain_prompt() {
    let dir = TempDir::new().expect("tempdir");
    let cache = TempDir::new().expect("cache dir");
    let prompt = compose_system_prompt(dir.path(), cache.path(), 1000);
    assert_eq!(prompt, SESSION_SYSTEM_PROMPT);
}

#[test]
fn missing_workspace_degrades_to_the_plain_prompt() {
    let cache = TempDir::new().expect("cache dir");
    let prompt = compose_system_prompt(Path::new("no-such-root-zq"), cache.path(), 1000);
    assert_eq!(prompt, SESSION_SYSTEM_PROMPT);
}
