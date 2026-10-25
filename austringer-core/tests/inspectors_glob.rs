//! Glob engine tests (todo 7): the fail-closed matcher's matrix —
//! separators, zero-directory `**/` cases, classes, escapes, unicode,
//! long (\>260-char) Windows paths, glob metacharacters in candidate
//! literals, and malformed-pattern rejection (never a permissive match).

use austringer_core::inspectors::glob::Glob;
use austringer_core::inspectors::permission::{Effect, Rule};

#[test]
fn glob_engine_matrix() {
    // `*` stays inside one path segment; `**` crosses separators.
    let star = Glob::new("*.pem").expect("valid");
    assert!(star.matches("k.pem"));
    assert!(!star.matches("dir/k.pem"));
    let deep = Glob::new("src/**").expect("valid");
    assert!(deep.matches("src/main.rs"));
    assert!(deep.matches("src/a/b/c.rs"));
    assert!(!deep.matches("srcx/a.rs"));
    let mid = Glob::new("a/**/z.txt").expect("valid");
    assert!(mid.matches("a/z.txt"));
    assert!(mid.matches("a/b/c/z.txt"));
    // Zero-directory `**/` prefix: `**/x` also matches a bare `x`.
    let anywhere = Glob::new("**/*.pem").expect("valid");
    assert!(anywhere.matches("k.pem"));
    assert!(anywhere.matches("deep/dir/k.pem"));

    // Prefix semantics (no dotfile special-casing: "*" matches ".env").
    let env = Glob::new(".env*").expect("valid");
    assert!(env.matches(".env"));
    assert!(env.matches(".env.local"));
    let any = Glob::new("*").expect("valid");
    assert!(any.matches(".env"));
    assert!(any.matches(""));

    // Single-char and classes.
    assert!(Glob::new("?.pem").expect("valid").matches("a.pem"));
    assert!(!Glob::new("?.pem").expect("valid").matches("ab.pem"));
    assert!(Glob::new("[a-c]x").expect("valid").matches("bx"));
    assert!(!Glob::new("[a-c]x").expect("valid").matches("dx"));
    assert!(Glob::new("[!a]x").expect("valid").matches("dx"));
    assert!(!Glob::new("[!a]x").expect("valid").matches("ax"));
    // Classes never cross the separator.
    assert!(!Glob::new("[!a]*").expect("valid").matches("a/b"));

    // Escaped metacharacters match literally.
    let esc = Glob::new(r"a\*").expect("valid");
    assert!(esc.matches("a*"));
    assert!(!esc.matches("abc"));

    // Case sensitivity is deliberate and documented.
    assert!(!Glob::new("*.PEM").expect("valid").matches("k.pem"));
}

#[test]
fn glob_normalizes_windows_separators_in_candidates_only() {
    let g = Glob::new("C:/temp/**").expect("valid");
    assert!(g.matches(r"C:\temp\x\y.txt"));
    // The pattern keeps `/` separators; `\` in a pattern is an escape.
    assert!(
        Glob::new(r"deep\ name")
            .expect("valid")
            .matches("deep name")
    );
}

#[test]
fn glob_unicode_and_long_paths() {
    assert!(Glob::new("*.pem").expect("valid").matches("鍵.pem"));
    assert!(Glob::new("漢/*").expect("valid").matches("漢/字"));
    assert!(
        Glob::new("*credentials*")
            .expect("valid")
            .matches("my-credentials.yml")
    );
    // Metacharacters in the candidate literal are ordinary characters.
    assert!(Glob::new("*.pem").expect("valid").matches("report[1].pem"));
    assert!(Glob::new("a?.txt").expect("valid").matches("a[.txt"));

    // >260-char Windows path: 278 chars of segments under `a/**`, and the
    // zero-directory `/**/` case (`a/**/z.txt` also matches `a/z.txt`).
    let long_middle = "seg".repeat(90);
    let long_path = format!("a/{long_middle}/z.txt");
    assert_eq!(long_path.len(), 278);
    assert!(Glob::new("a/**/z.txt").expect("valid").matches(&long_path));
}

#[test]
fn malformed_globs_fail_closed() {
    for bad in ["", "[unclosed", "trailing\\", "[]", "[z-a]"] {
        assert!(
            Glob::new(bad).is_err(),
            "pattern {bad:?} must fail compilation, never match"
        );
    }
    // A pattern that failed compilation cannot be constructed as a rule
    // either: rule constructors propagate the typed error.
    assert!(Rule::path(Effect::Deny, "read_file", "[unclosed").is_err());
    assert!(Rule::tool(Effect::Allow, "").is_err());
}
