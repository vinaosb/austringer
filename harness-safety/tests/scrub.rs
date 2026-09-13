//! Shell-env secret scrub tests (todo 26).

#![cfg(test)]

use harness_safety::scrub::{EnvScrub, ScrubbedEnv, scrub_env};

fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn key_secret_token_vars_are_dropped_by_default() {
    let input = vars(&[
        ("PATH", "/bin"),
        ("ANTHROPIC_API_KEY", "sk-ant-1"),
        ("GITHUB_TOKEN", "ghp_1"),
        ("DB_PASSWORD", "hunter2"),
        ("MY_SECRET_VALUE", "s"),
        ("CI", "true"),
    ]);
    let scrubbed = scrub_env(&input, &EnvScrub::default());
    assert_eq!(
        scrubbed.dropped,
        ["ANTHROPIC_API_KEY", "GITHUB_TOKEN", "MY_SECRET_VALUE"]
    );
}

#[test]
fn secret_substring_matches_case_insensitive() {
    let input = vars(&[("my_token_name", "x"), ("SECRET", "y")]);
    let scrubbed = scrub_env(&input, &EnvScrub::default());
    assert_eq!(scrubbed.dropped.len(), 2);
}

#[test]
fn typed_exception_list_keeps_named_vars() {
    let input = vars(&[
        ("TOKENS_DIR", "/tok"),
        ("PATH", "/bin"),
        ("SECRET_FILE", "/s"),
    ]);
    let scrub = EnvScrub::with_exceptions(["TOKENS_DIR"]);
    let scrubbed = scrub_env(&input, &scrub);
    assert!(scrubbed.dropped.contains(&"SECRET_FILE".to_string()));
    assert!(!scrubbed.dropped.contains(&"TOKENS_DIR".to_string()));
    assert!(scrubbed.kept.iter().any(|(k, _)| k == "TOKENS_DIR"));
}

#[test]
fn inherit_core_env_survives_scrub() {
    let input = vars(&[
        ("PATH", "/usr/bin"),
        ("SYSTEMROOT", r"C:\Windows"),
        ("TEMP", "/tmp"),
        ("TMP", "/tmp"),
        ("PATHEXT", ".COM"),
        ("COMSPEC", "cmd.exe"),
        ("WINDIR", r"C:\Windows"),
        ("HOME", "/home/u"),
        ("LANG", "en_US.UTF-8"),
        ("API_TOKEN", "x"),
    ]);
    let scrubbed = scrub_env(&input, &EnvScrub::default());
    for core in [
        "PATH",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "PATHEXT",
        "COMSPEC",
        "WINDIR",
        "HOME",
        "LANG",
    ] {
        assert!(
            scrubbed.kept.iter().any(|(k, _)| k == core),
            "core var {core} must survive"
        );
    }
    assert_eq!(scrubbed.dropped, ["API_TOKEN"]);
}

#[test]
fn scrubbed_env_reports_what_was_removed() {
    let input = vars(&[("A_KEY", "1"), ("PATH", "/bin")]);
    let scrubbed: ScrubbedEnv = scrub_env(&input, &EnvScrub::default());
    assert_eq!(scrubbed.dropped.len(), 1);
    assert_eq!(scrubbed.kept.len(), 1);
}
